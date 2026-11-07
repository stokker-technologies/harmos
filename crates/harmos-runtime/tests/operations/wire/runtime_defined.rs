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
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::*;
use harmos_runtime::operations::{Backoff, Jitter, RetryPolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Observer(Arc<Owner>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::observers::ObserverBackend for Observer {
    fn definition(&self) -> Result<harmos_runtime::observers::ObserverDefinition> {
        Ok(harmos_runtime::observers::ObserverDefinition {
            id: "provider.runtime_watch".into(),
            streams: vec![],
            demand_keys: vec![],
        })
    }
    async fn run(
        &self, resources: Arc<harmos_sdk::ExecutionContext>, mut activation: harmos_runtime::observers::ObserverActivation,
    ) -> Result<()> {
        let owner = resources.resource::<Owner>().unwrap();
        assert!(Arc::ptr_eq(&owner, &self.0));
        activation.ready();
        owner.started.notify_one();
        activation.stopping().await;
        owner.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/* === Public Functions === */

pub(super) fn safe_contract() -> harmos_runtime::operations::Contract {
    let original = WireDeclaration::of::<ControlDeclaration>().contract().unwrap();
    let mut definition = original.operation().clone();
    definition.id = harmos_runtime::Identifier::new("fixture.runtime_safe").unwrap();
    definition.replay = harmos_runtime::operations::ReplaySafety::Idempotent;
    definition.policy.retries = RetryPolicy::Never;
    definition.visibility = harmos_runtime::operations::Visibility::Public;
    harmos_runtime::operations::Contract::from_schema(definition, original.codec(), original.schema().clone(), &Default::default()).unwrap()
}

/* === Private Functions === */

fn owned_definitions(revision: &str) -> PluginDefinitions {
    let mut definitions: Vec<_> = definitions(revision).definitions().cloned().collect();
    let provider = definitions.iter_mut().find(|definition| definition.id == "provider").unwrap();
    provider.contributions.operations.push(safe_contract().operation().clone());
    PluginDefinitions::new(definitions, []).unwrap()
}

#[tokio::test]
async fn owned_binding_preserves_admission_and_generation() {
    // Start Runtime And Stage An Owned Codec
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_resources(NonZeroUsize::new(4096).unwrap(), owner.clone(), None).await;
    let mut host = Host {
        operations: runtime.operations.clone(),
        owner: owner.clone(),
        scope: None,
        callers: BTreeMap::new(),
        runtime_owned: true,
    };
    let mut manager = PluginManager::new(owned_definitions("first")).unwrap();
    manager.initialize(&mut host).await.unwrap();
    let scope = host.scope.as_ref().unwrap().clone();
    let contract = WireDeclaration::of::<ControlDeclaration>().contract().unwrap();
    let callback = runtime.operations.capture_callback(&scope, &contract).unwrap();
    let route = format!("provider.{}", ControlDeclaration::ID);

    // Invoke Through Existing Encoded Admission
    let response =
        runtime.operations.submit_encoded_with(&route, Text::from("owned").encode(), Some(RetryPolicy::Never)).unwrap().await.unwrap();
    assert_eq!(String::decode(&response).unwrap(), "owned");
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);

    // Refuse Unknown Replay Safety Before Executing
    let retry = RetryPolicy::OnTransient {
        max_retries: 1,
        backoff: Backoff::Fixed {
            delay: Duration::from_millis(1),
            jitter: Jitter::None,
        },
    };
    assert!(runtime.operations.submit_encoded_with(&route, Text::from("unsafe").encode(), Some(retry)).is_err());
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);

    // Preserve Host Visibility For Plugin Callers And Callbacks
    let caller = runtime.operations.caller(&host.callers["provider"]).unwrap();
    assert!(matches!(caller.submit_encoded_with(&route, Text::from("denied").encode(), None), Err(Error::OperationUnauthorized { .. })));
    assert!(matches!(
        callback.submit_with_retry(NonZeroUsize::new(64).unwrap(), |_| Ok(Text::from("denied").encode()), None),
        Err(Error::OperationUnauthorized { .. })
    ));

    // Retire The Original Generation Without Rebinding Its Callback
    owner.prepared.store(0, Ordering::SeqCst);
    manager.reload(owned_definitions("second"), &mut host).await.unwrap();
    assert_ne!(scope.generation(), host.scope.as_ref().unwrap().generation());
    let encoded = AtomicBool::new(false);
    assert!(
        callback
            .submit_with_retry(
                NonZeroUsize::new(64).unwrap(),
                |_| {
                    encoded.store(true, Ordering::SeqCst);
                    Ok(Text::from("stale").encode())
                },
                None
            )
            .is_err()
    );
    assert!(!encoded.load(Ordering::SeqCst));
    assert!(runtime.operations.submit_encoded(&route, vec![0xff]).is_err());

    // Shut Down The Admitted Generation
    manager.shutdown(&mut host).await.unwrap();
    assert!(runtime.operations.submit_encoded(&route, Text::from("closed").encode()).is_err());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn owned_binding_retries_and_cancels_under_native_policy() {
    // Initialize An Owned Replay-Safe Binding
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_resources(NonZeroUsize::new(4096).unwrap(), owner.clone(), None).await;
    let mut host = Host {
        operations: runtime.operations.clone(),
        owner: owner.clone(),
        scope: None,
        callers: BTreeMap::new(),
        runtime_owned: true,
    };
    let mut manager = PluginManager::new(owned_definitions("first")).unwrap();
    manager.initialize(&mut host).await.unwrap();
    let route = "provider.fixture.runtime_safe";
    let retry = RetryPolicy::OnTransient {
        max_retries: 1,
        backoff: Backoff::Fixed {
            delay: Duration::from_millis(1),
            jitter: Jitter::None,
        },
    };

    // Override One Call And Retain Recorded Attempts
    let invocation = runtime.operations.submit_encoded_with(route, Text::from("retry").encode(), Some(retry)).unwrap();
    invocation.settle().await.unwrap();
    assert_eq!(invocation.status().attempt(), 1);
    assert_eq!(String::decode(&invocation.await.unwrap()).unwrap(), "retry");
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 2);

    // The Declaration Still Has Its Original No-Retry Default
    owner.prepared.store(0, Ordering::SeqCst);
    assert!(runtime.operations.submit_encoded(route, Text::from("retry").encode()).unwrap().await.is_err());
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);

    // A Retained Callback Applies The Same Per-Call Override
    owner.prepared.store(0, Ordering::SeqCst);
    let callback = runtime.operations.capture_callback(host.scope.as_ref().unwrap(), &safe_contract()).unwrap();
    let invocation = callback.submit_with_retry(NonZeroUsize::new(64).unwrap(), |_| Ok(Text::from("retry").encode()), Some(retry)).unwrap();
    invocation.settle().await.unwrap();
    assert_eq!(invocation.status().attempt(), 1);
    assert_eq!(String::decode(&invocation.await.unwrap()).unwrap(), "retry");
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 2);

    // Cancel And Join The Same Admitted Native Instance
    let invocation = runtime.operations.submit_encoded(route, Text::from("block").encode()).unwrap();
    owner.started.notified().await;
    assert!(invocation.cancel().await.is_err());

    // A Retry Override Cannot Extend The Operation's Attempt Deadline
    let invocation = runtime.operations.submit_encoded_with(route, Text::from("block").encode(), Some(retry)).unwrap();
    assert!(matches!(invocation.await, Err(Error::OperationRefused(harmos_runtime::Refusal::TimedOut))));
    manager.shutdown(&mut host).await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn owned_observer_uses_plugin_resources_and_retires_with_generation() {
    // Initialize The Plugin Generation
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_resources(NonZeroUsize::new(4096).unwrap(), owner.clone(), None).await;
    let mut host = Host {
        operations: runtime.operations.clone(),
        owner: owner.clone(),
        scope: None,
        callers: BTreeMap::new(),
        runtime_owned: true,
    };
    let mut manager = PluginManager::new(owned_definitions("first")).unwrap();
    manager.initialize(&mut host).await.unwrap();
    let scope = host.scope.as_ref().unwrap().clone();

    // Stage And Activate Over A Distinct Resource Snapshot
    let mut resources = harmos_sdk::__private::ResourceSet::new();
    resources.insert(owner.clone()).unwrap();
    let resources = Arc::new(harmos_sdk::ExecutionContext::new(resources, Default::default()));
    runtime.operations.stage_plugin_observers(&resources, &scope, [Observer(owner.clone())]).unwrap().commit().unwrap();
    let observer = runtime.operations.submit_resident_in("provider.runtime_watch", resources.clone()).await.unwrap();
    owner.started.notified().await;

    // Retirement Stops The Observer And Fences Its Old Scope
    manager.shutdown(&mut host).await.unwrap();
    observer.release().await.unwrap();
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    assert!(runtime.operations.stage_plugin_observers(&resources, &scope, [Observer(owner.clone())]).is_err());
    runtime.shutdown().await.unwrap();
}

/* === Tests === */

#[test]
fn owned_contract_derives_identity_and_validates_shape() {
    let original = WireDeclaration::of::<ControlDeclaration>().contract().unwrap();
    let owned = safe_contract();
    assert_ne!(original.fingerprint(), owned.fingerprint());
    assert!(harmos_runtime::operations::Contract::new(
        owned.operation().clone(), original.fingerprint(), owned.codec(), owned.schema().clone(),
    ).is_err());
    let mut schema = owned.schema().clone();
    schema.root = schema.definitions.len();
    assert!(
        harmos_runtime::operations::Contract::from_schema(owned.operation().clone(), owned.codec(), schema, &Default::default(),).is_err()
    );
}
