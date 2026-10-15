/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeMap;
use std::sync::Mutex;

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */
const MIB: usize = 1024 * 1024;
// More Members Than Guaranteed Ceilings The Default 16 MiB Pool Can Hold.
const MEMBERS: usize = 9;
const WORK: Duration = Duration::from_millis(20);
const FIRST_BACKOFF: Duration = Duration::from_millis(10);
const LAST_BACKOFF: Duration = Duration::from_millis(160);

/* === Macros === */

/* === Type Aliases === */
type Refusals = Mutex<BTreeMap<(&'static str, &'static str), usize>>;

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn page() -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(1024).unwrap(),
    }
}

fn projection() -> NonZeroUsize {
    NonZeroUsize::new(MIB).unwrap()
}

// Rendered Output Each Projection Keeps: Three Members Overfill The Slack
// Their Guaranteed Ceilings Leave In The Pool, Yet Each Fits Its Own Ceiling.
fn retained() -> Vec<u8> {
    vec![0; 3 * MIB / 4]
}

// Records A Refusal By Step; Any Other Failure Fails The Story.
fn refused<T>(refusals: &Refusals, step: &'static str, result: Result<T, Error>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(Error::ResourceExhausted { resource }) => {
            *refusals
                .lock()
                .unwrap()
                .entry((step, resource))
                .or_default() += 1;
            None
        }
        Err(error) => panic!("{step} failed: {error}"),
    }
}

// One Report Member: A Guaranteed View, A Page, Projections Under The View,
// Held While Rendering. A Refusal Drops Everything And Retries With Backoff.
async fn member(storage: StorageManager, limits: QueryLimits, refusals: Arc<Refusals>) {
    let mut backoff = FIRST_BACKOFF;
    loop {
        if attempt(&storage, limits, &refusals).await.is_some() {
            return;
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(LAST_BACKOFF);
    }
}

async fn attempt(storage: &StorageManager, limits: QueryLimits, refusals: &Refusals) -> Option<()> {
    let view = refused(refusals, "open", storage.view_with_limits(limits).await)?;
    let mut reader = storage.query_in(&view, record_query(page())).unwrap();
    let page = refused(refusals, "page", reader.next_page().await)?.unwrap();
    let rows = refused(
        refusals,
        "page projection",
        page.try_project(projection(), |_, _| Ok(retained())),
    )?;
    let empty = refused(
        refusals,
        "production",
        storage.produce(NonZeroUsize::new(64).unwrap(), |_| Ok(Vec::<u8>::new())),
    )?;
    let owned = refused(
        refusals,
        "owned projection",
        view.project_owned(
            empty.try_into_value().unwrap(),
            projection(),
            |mut value, _| {
                value.extend(retained());
                Ok(value)
            },
        ),
    )?;
    tokio::time::sleep(WORK).await;
    drop((owned, rows, page, reader, view));
    Some(())
}

/* === Tests === */
#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("guaranteed_members_settle_under_contention", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "saved").await;
                    f.storage.flush().await.unwrap();
                    let refusals = Arc::new(Refusals::default());
                    let started = tokio::time::Instant::now();
                    let members = (0..MEMBERS)
                        .map(|_| {
                            tokio::spawn(member(f.storage.clone(), f.execution, refusals.clone()))
                        })
                        .collect::<Vec<_>>();
                    for member in members {
                        member.await.unwrap();
                    }
                    let elapsed = started.elapsed();
                    let refusals = refusals.lock().unwrap().clone();
                    eprintln!("Settled In {elapsed:?} With Refusals {refusals:?}");
                    // Admitted Members Never Retry: Their Ceilings Cover Their Reads.
                    assert!(refusals.keys().all(|(step, _)| *step == "open"));
                    tokio::time::timeout(Duration::from_secs(2), async {
                        while f.storage.status().read_bytes != 0 {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    f.finish().await;
                });
        });
    }
}
