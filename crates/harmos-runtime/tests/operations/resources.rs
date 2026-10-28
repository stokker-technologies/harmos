/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use std::time::Duration;

use super::{Lanes, app::App};
use harmos_runtime::operations::RetryPolicy;
use harmos_runtime::{Runtime, state::Position};

struct Bench {
    device: String,
    reads: AtomicUsize,
}

#[harmos_runtime::operation(resources = [Bench], operations = [ReadContractDeclaration], response = Reading,
        id = "resources.read",
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

impl harmos_runtime::Operation for ReadContractDeclaration {
    type Error = harmos_runtime::operations::InvocationError;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Reading>, Self::Error> {
        let resources = ctx.resource::<Bench>();
        let seen = u64::try_from(resources.reads.fetch_add(1, Ordering::SeqCst)).expect("read counts fit in u64");
        if self.nested {
            let child = ctx.operation(ReadContractDeclaration { nested: false }).await?;
            assert_eq!(child.device, resources.device);
            assert_eq!(child.seen, seen + 1);
        }
        Ok(Reading { device: resources.device.clone(), seen }.into())
    }
}

#[tokio::test]
async fn needs_lacks_registration() {
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    assert_eq!(runtime.shutdown().await.unwrap(), Position::ORIGIN);
}

#[tokio::test]
async fn nested_resources_shared() {
    let resources = Arc::new(Bench {
        device: "sim://rig-a".to_owned(),
        reads: AtomicUsize::new(0),
    });
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Bench>(resources.clone());
    })
    .await
    .unwrap();
    runtime.operations.register::<Operations>(Lanes::Open).await.unwrap();

    let parent = runtime.operation(ReadContractDeclaration { nested: true }).start().await.unwrap().await.unwrap();
    assert_eq!(parent.device, "sim://rig-a");
    assert_eq!(parent.seen, 0);
    let following = runtime.operation(ReadContractDeclaration { nested: false }).start().await.unwrap().await.unwrap();
    assert_eq!(following.seen, 2, "parent, child and following attempt share one value");
    runtime.shutdown().await.unwrap();
}
