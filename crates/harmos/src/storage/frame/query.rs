/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::num::NonZeroUsize;
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::super::manager::{ErasedCatalog, MemoryReservation};
use super::super::query::{execution_timeout, sealed};
use super::super::{
    QueryLimits, QueryOptions, QueryValue, ReadOrder, SharedQueryValue, StorageManager,
    StorageQuery, StorageView,
};
use super::execute::{plan, reuses_left};
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct FrameQuery {
    pub frame: Frame,
    pub options: QueryOptions,
}
pub struct FrameResult {
    frame: Frame,
    view: StorageView,
    page: ReadLimits,
    explain: Explain,
    decoded_bytes: NonZeroUsize,
    cached: Option<SharedQueryValue<Collected>>,
    _reservation: MemoryReservation,
}
// Pages One Frame Scan From The Primary Backend Under The Manager's Bounds.
struct BackendScan<'a> {
    manager: &'a StorageManager<ErasedCatalog>,
    view: &'a ReadView,
    frame: &'a Frame,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl sealed::Supported for FrameQuery {
    fn options_mut(&mut self) -> Option<&mut QueryOptions> {
        Some(&mut self.options)
    }
}
impl<C> StorageQuery<C> for FrameQuery {
    type Reader = FrameResult;
    fn options(&self) -> &QueryOptions {
        &self.options
    }

    fn preflight(&self, manager: &StorageManager<C>, _limits: QueryLimits) -> Result<()> {
        self.frame.check()?;
        if self.options.after != 0
            || self.options.through.is_some()
            || self.options.order != ReadOrder::Ascending
        {
            return Err(invalid(
                "frame cuts and ordering must be explicit in scans and joins",
            ));
        }
        let _reservation = manager.reserve_read_now(self.frame.memory_size(), 1)?;
        check_orders(&self.frame, |scan, order| {
            manager.inner.primary.supports_scan_order(scan, order)
        })
    }

    fn open(
        self,
        view: StorageView,
        page: ReadLimits,
        remaining: Duration,
    ) -> Result<Self::Reader> {
        let manager = view.manager();
        manager.check_query_open(&view, &self.options, page, remaining)?;
        let computation = view
            .computation_with(Some(remaining), Duration::ZERO)?
            .ok_or(invalid("frame requires bounded saved view"))?;
        let (_pin, read_view) = view.pin()?;
        self.frame.check_view(&read_view)?;

        // Explain Is The Only Memory Figure; It Must Fit The Working Allowance.
        let explain = self.frame.explain(page)?;
        let execution = read_view
            .execution_limits()
            .ok_or(invalid("frame requires query limits"))?;
        if explain.peak_bytes > execution.memory_bytes.get() {
            return Err(invalid(
                "explained frame memory exceeds query working memory",
            ));
        }
        check_orders(&self.frame, |scan, order| {
            manager.inner.primary.supports_scan_order(scan, order)
        })?;
        let decoded_bytes = manager.query_decoded_bytes(&self.options)?;
        let reservation = view.reserve_read(self.frame.memory_size(), 1)?;
        computation.finish(FrameResult {
            frame: self.frame,
            view,
            page,
            explain,
            decoded_bytes,
            cached: None,
            _reservation: reservation,
        })
    }
}
impl FrameResult {
    pub fn view(&self) -> &StorageView {
        &self.view
    }

    pub fn explain(&self) -> &Explain {
        &self.explain
    }

    pub fn decoded_bytes(&self) -> NonZeroUsize {
        self.decoded_bytes
    }

    pub async fn read(&mut self) -> Result<SharedQueryValue<Collected>> {
        let computation = self
            .view
            .computation()?
            .ok_or(invalid("frame requires bounded saved view"))?;
        let (_pin, read_view) = self.view.pin()?;
        self.frame.check_view(&read_view)?;
        if let Some(cached) = &self.cached {
            return computation.finish(cached.clone());
        }

        // Peak Memory Is Charged Before The First Page Is Requested.
        let manager = self.view.manager();
        let reservation = self.view.reserve_read(self.explain.peak_bytes, 1)?;
        let collected = tokio::time::timeout(computation.timeout(), async {
            let _activity = self.view.activity()?;
            let _lifecycle = manager.inner.lifecycle.lock().await;
            let mut source = BackendScan {
                manager,
                view: &read_view,
                frame: &self.frame,
            };
            let mut check = || computation.check();
            collect(
                &self.frame,
                &mut source,
                &self.explain,
                self.page,
                &mut check,
            )
            .await
        })
        .await
        .map_err(|_| execution_timeout())??;
        collected.check(&self.frame)?;
        if collected.memory_size() > self.decoded_bytes.get() {
            return Err(invalid("collected frame exceeds decoded output allowance"));
        }
        computation.check()?;
        let value = QueryValue::retain(collected, reservation)?.try_into_shared()?;
        self.cached = Some(value.clone());
        computation.finish(value)
    }
}
impl ScanSource for BackendScan<'_> {
    fn page(
        &mut self,
        scan: usize,
        order: &ScanOrder,
        cursor: Option<&ScanCursor>,
        limits: ReadLimits,
    ) -> impl Future<Output = Result<ScanPage>> + Send {
        let manager = self.manager;
        let view = self.view;
        let scan = &self.frame.scans[scan];
        async move {
            manager
                .inner
                .bounded(
                    manager
                        .inner
                        .primary
                        .scan_ordered(view, scan, order, cursor, limits),
                )
                .await
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
// Every Pass Order And Every Separate Right Order Must Be Supported Up Front.
fn check_orders(frame: &Frame, supports: impl Fn(&Scan, &ScanOrder) -> bool) -> Result<()> {
    for pass in plan(frame)? {
        if !supports(&frame.scans[pass.source], &pass.order) {
            return Err(invalid("backend does not support the required scan order"));
        }
        for &join in &pass.joins {
            let as_of = &frame.as_ofs[join];
            if reuses_left(as_of) {
                continue;
            }
            let order = execute::right_order(as_of);
            if !supports(&frame.scans[as_of.right], &order) {
                return Err(invalid("backend does not support the required scan order"));
            }
        }
    }
    Ok(())
}

/* === Tests === */
