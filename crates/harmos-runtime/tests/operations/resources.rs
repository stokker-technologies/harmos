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
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::operations::{Handler, RetryPolicy};
use harmos_runtime::{Error, Runtime, state::Position};

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

#[harmos_runtime::operation(response = Reading,
        id = "resources_read",
        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
        },
    )]
struct ReadContractDeclaration {
    nested: bool,
}

// The Device A Read Saw And How Many Reads Preceded It.
#[harmos_runtime::message]
struct Reading {
    device: String,
    seen: u64,
}

#[harmos_runtime::catalog]
enum Operations {
    ReadContractDeclaration(ReadContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl Handler for Read {
    type Request = ReadContractDeclaration;
    type Response = <ReadContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = Error;
    type StreamEndpoints = ();

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
        let resources = context.resource::<Bench>().expect("fixture registers its dependency");
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let constructed: Result<Self, Self::Error> = async move { Ok(Self { resources }) }.await;
        let mut operation = constructed?;
        let started: Result<(), Error> = async {
            let operation = &mut operation;
            let scope = context.scope();
            assert!(Arc::ptr_eq(&operation.resources, scope.resources::<App<Arc<Bench>>>()));
            operation.resources.initialized.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        .await;
        started?;
        Ok(operation)
    }

    async fn run(
        &mut self, context: harmos_runtime::OperationContext, request: &mut ReadContractDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<Reading>, Error> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<Reading, Self::Error> = async move {
            let seen = u64::try_from(self.resources.reads.fetch_add(1, Ordering::SeqCst)).expect("read counts fit in u64");
            if request.nested {
                let child = context.submit::<ReadContractDeclaration>(ReadContractDeclaration { nested: false })?.await?;
                assert_eq!(child.device, self.resources.device);
                assert_eq!(child.seen, seen + 1);
            }
            Ok(Reading {
                device: self.resources.device.clone(),
                seen,
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn cleanup(&mut self) -> Result<(), Error> {
        self.resources.finalized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn needs_lacks_registration() {
    let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
    assert_eq!(runtime.stop().await.unwrap(), Position::ORIGIN);
}

#[tokio::test]
async fn child_own_resources() {
    let resources = Arc::new(Bench {
        device: "sim://rig-a".to_owned(),
        reads: AtomicUsize::new(0),
        initialized: AtomicUsize::new(0),
        finalized: AtomicUsize::new(0),
    });
    let runtime = Runtime::<App<Arc<Bench>>>::builder((), resources.clone()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(resources.clone()).unwrap();
                dependencies
            })
            .implementation::<ReadContractDeclaration, Read>(),
            Lanes::Open,
        ))
        .await
        .unwrap();

    let parent = runtime.operation(ReadContractDeclaration { nested: true }).start().await.unwrap().await.unwrap();
    assert_eq!(parent.device, "sim://rig-a");
    assert_eq!(parent.seen, 0);
    let following = runtime.operation(ReadContractDeclaration { nested: false }).start().await.unwrap().await.unwrap();
    assert_eq!(following.seen, 2, "parent, child and following attempt share one value");
    assert_eq!(resources.initialized.load(Ordering::SeqCst), 3);
    assert_eq!(resources.finalized.load(Ordering::SeqCst), 3);
    runtime.stop().await.unwrap();
}
