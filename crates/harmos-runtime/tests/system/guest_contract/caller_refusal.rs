/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Preserve Wrong-Caller Failures Across Guest Imports

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{contract::EchoOperation, support};
use harmos_operations::execution::operations::OperationFactory;
use harmos_protocol::model::{Classification, Failure};
use harmos_runtime::{Runtime, clock::Clock, operations::Lane};
use std::{sync::Arc, time::SystemTime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn guest_import_preserves_the_wrong_caller_failure() {
    let _report = crate::report::system();
    // Narrow the Host Operation Before Admitting the Guest's Matching Requirement.
    let (clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let lane = Lane::default();
    let (storage, entries, root) = support::entries();
    let runtime = Runtime::<crate::support::Bank>::launch_with(crate::support::Ledger::default(), (), |setup| {
        setup.resource::<support::ProviderOwner>(Arc::new(support::ProviderOwner::default()));
        setup.resource(root);
        setup.settings().storage(&storage).plugins(entries.clone());
        setup.settings().clock(clock);
        setup.settings().register(crate::support::catalog());
        setup.operation(OperationFactory::protobuf::<support::Publisher>().unwrap(), lane);
        setup.operation(OperationFactory::protobuf::<support::Provider>().unwrap(), lane);
        setup.composition("[callers]\n\"harmos.host.v1.Time/Get\" = \"CLIENTS\"\n").unwrap();
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: std::num::NonZeroUsize::new(4096),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    let runtime = Arc::new(runtime);
    entries.start(&runtime).await.unwrap();
    let host = support::host(&runtime);
    let mut settings = support::settings("caller-refusal");
    settings.host_operations.push(harmos_runtime::Identifier::new("harmos.host.v1.Time/Get").unwrap());
    runtime.plugin.register_guest(host.load(&support::sandbox()).unwrap(), settings, lane).await.unwrap();
    runtime.plugin.initialize().await.unwrap();

    // The Real WebAssembly Import Returns the Same Failure as the Native Carriers.
    let error = support::call(&runtime.operations, EchoOperation { say: "caller-refusal".into() }).await.unwrap_err();
    let failure = Failure::from(error.failure().unwrap());
    assert_eq!(failure.classification(), Classification::Refused);
    assert_eq!(failure.code, harmos_operations::WRONG_CALLER.to_string());
    assert_eq!(failure.operation, "harmos.host.v1.Time/Get");
    assert_eq!(failure.action, "Call");
    assert!(failure.message.contains(&failure.operation));

    // The Guest Remains Usable After the Refusal.
    let response = support::call(&runtime.operations, EchoOperation { say: "after refusal".into() }).await.unwrap();
    assert_eq!(response.said, "heard after refusal");
    runtime.shutdown().await.unwrap();
}
