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
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Lanes, app::App};
use harmos::operations::{Context, Operation, OperationPolicy, RetryPolicy};
use harmos::{Error, Position, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Bench {
    device: String,
    reads: AtomicUsize,
    initialized: AtomicUsize,
    finalized: AtomicUsize,
}

struct Read {
    resources: Arc<Bench>,
}

/* === Enums === */
#[harmos::catalog]
enum Operations {
    #[operation(
        request = bool,
        response = (String, usize),
        id = "resources.read",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
        },
    )]
    ReadContract,
}

/* === Traits === */

/* === Implementations === */
impl Operation for Read {
    type Declaration = ReadContract;
    type Resources = Bench;
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    fn new(resources: Arc<Bench>) -> Result<Self, Self::Error> {
        Ok(Self { resources })
    }

    async fn initialize<'ctx>(&mut self, context: Self::Context<'ctx>) -> Result<(), Error> {
        let scope = context.scope();
        assert!(Arc::ptr_eq(
            &self.resources,
            scope.resources::<App<Arc<Bench>>>()
        ));
        self.resources.initialized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        nested: &mut bool,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<(String, usize), Error> {
        let seen = self.resources.reads.fetch_add(1, Ordering::SeqCst);
        if *nested {
            let child = context.submit::<ReadContract>(false)?.await?;
            assert_eq!(child.0, self.resources.device);
            assert_eq!(child.1, seen + 1);
        }
        Ok((self.resources.device.clone(), seen))
    }

    async fn finalize(&mut self) -> Result<(), Error> {
        self.resources.finalized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("needs_lacks_registration", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
                    assert_eq!(runtime.stop().await.unwrap(), Position::ORIGIN);
                });
        });
    }
    {
        scenario_runtime::check("child_own_resources", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let resources = Arc::new(Bench {
                        device: "sim://rig-a".to_owned(),
                        reads: AtomicUsize::new(0),
                        initialized: AtomicUsize::new(0),
                        finalized: AtomicUsize::new(0),
                    });
                    let runtime = Runtime::<App<Arc<Bench>>>::builder((), resources.clone())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    runtime
                        .operations
                        .register_operation::<Read>("app", Lanes::Open, resources.clone())
                        .unwrap();

                    let parent = runtime
                        .operations
                        .submit::<ReadContract>(true)
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(parent.0, "sim://rig-a");
                    assert_eq!(parent.1, 0);
                    let following = runtime
                        .operations
                        .submit::<ReadContract>(false)
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(
                        following.1, 2,
                        "parent, child and following attempt share one value"
                    );
                    assert_eq!(resources.initialized.load(Ordering::SeqCst), 3);
                    assert_eq!(resources.finalized.load(Ordering::SeqCst), 3);
                    runtime.stop().await.unwrap();
                });
        });
    }
}
