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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
// Third Party Imports

// Local Imports
use super::contract;
use crate::witness::Witness;
use contract::{POLICY, Read, Readings, Step};
use harmos::operations::{Operation, OperationPolicy, WireTable};
use harmos::sidecar::{Refusal, Supervisor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Provider {
    resources: Arc<Resources>,
}

pub(crate) struct Resources {
    marks: Arc<Witness>,
    initialized: AtomicBool,
}

struct Reading {
    resources: Arc<Resources>,
    phase: Option<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for Provider {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos::plugin(id = "process-provider", provides = Readings, requires = ())]
impl harmos::Plugin for Provider {
    type Context = Supervisor;
    type Config = String;
    type Error = Refusal;
    type Resources = Resources;

    fn prepare(key: String) -> Result<Self, Refusal> {
        Ok(Self {
            resources: Arc::new(Resources {
                marks: Witness::find(&key).ok_or_else(|| Refusal::of("the story witness is gone"))?,
                initialized: AtomicBool::new(false),
            }),
        })
    }

    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        self.resources.marks.mark("initialized");
        self.resources.initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn resources(&self) -> Arc<Resources> {
        Arc::clone(&self.resources)
    }

    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
        table.bind_operation::<Reading>(self.resources())
    }

    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        self.resources.initialized.store(false, Ordering::SeqCst);
        self.resources.marks.mark("shutdown");
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Refusal> {
        self.resources.marks.mark("cleaned");
        Ok(())
    }
}

impl Operation for Reading {
    type Declaration = Read;
    type Resources = Resources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;

    async fn initialize<'ctx>(
        resources: Arc<Resources>,
        _context: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        let constructed: Result<Self, Refusal> = async move {
            Ok(Self {
                resources,
                phase: None,
            })
        }
        .await;
        let mut operation = constructed?;
        let started: Result<(), Refusal> = async {
            let operation = &mut operation;
            operation.resources.marks.mark("read-initialized");
            Ok(())
        }
        .await;
        started?;
        Ok(operation)
    }

    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Step,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Step>, Refusal> {
        let result: ::core::result::Result<Step, Self::Error> = async move {
            self.phase = Some(request.phase);
            self.resources
                .marks
                .mark(&format!("read-prepared-{}", request.phase));
            if !self.resources.initialized.load(Ordering::SeqCst) {
                return Err(Refusal::of("the original provider has retired"));
            }
            self.resources
                .marks
                .mark(&format!("read-started-{}", request.phase));
            if request.phase == 2 && self.resources.marks.seen("hold-read-2") {
                self.resources.marks.marked("release-read-2").await;
            }
            Ok(request.clone())
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn cleanup(&mut self) -> Result<(), Refusal> {
        if !self.resources.initialized.load(Ordering::SeqCst) {
            return Err(Refusal::of(
                "the provider retired before child finalization",
            ));
        }
        match self.phase {
            Some(phase) => self
                .resources
                .marks
                .mark(&format!("read-finalized-{phase}")),
            None => self.resources.marks.mark("read-finalized-unstarted"),
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
/* === Tests === */
