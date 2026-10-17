/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Exercise Portable Plugin Registration And Generation Replacement.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;


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
async fn callback_requires_budget() {
    let runtime = harmos_runtime::Runtime::<crate::support::Bank>::builder(crate::support::Ledger::default(), ())
        .register(crate::support::catalog())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    let mut manager = PluginManager::new(candidate()).unwrap();
    let mut host = host(runtime.operations.clone());
    manager.initialize(&mut host).await.unwrap();
    let contract = WireDeclaration::of::<super::super::cases::Echo>().contract().unwrap();
    let target: harmos_runtime::operations::PluginCallback = runtime.operations.capture_callback(host.scope.as_ref().unwrap(), &contract).unwrap();
    let encoded = AtomicBool::new(false);
    let refused = target.submit_with(NonZeroUsize::new(64).unwrap(), |_| {
        encoded.store(true, Ordering::SeqCst);
        Ok(Text::from("value").encode_to_vec())
    });
    assert!(matches!(refused, Err(Error::InvalidInput { field: "operation preparation bytes", .. })));
    assert!(!encoded.load(Ordering::SeqCst));
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn callback_contract_generation() {
    let runtime = super::super::runtime_with_preparation(NonZeroUsize::new(128).unwrap()).await;
    let mut manager = PluginManager::new(candidate()).unwrap();
    let mut host = host(runtime.operations.clone());
    manager.initialize(&mut host).await.unwrap();
    let contract = WireDeclaration::of::<super::super::cases::Echo>().contract().unwrap();
    let target = runtime.operations.capture_callback(host.scope.as_ref().unwrap(), &contract).unwrap();
    let mut changed = WireDeclaration::of::<super::super::cases::Echo>();
    changed.version = 2;
    assert!(matches!(
        runtime.operations.capture_callback(host.scope.as_ref().unwrap(), &changed.contract().unwrap()),
        Err(Error::InvalidInput { field: "callback contract", .. })
    ));
    let mut definitions: Vec<_> = candidate().definitions().cloned().collect();
    definitions[0].configuration = br#"{"generation":2}"#.to_vec();
    manager.reload(PluginDefinitions::new(definitions, []).unwrap(), &mut host).await.unwrap();
    let encoded = AtomicBool::new(false);
    assert!(
        target
            .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                encoded.store(true, Ordering::SeqCst);
                Ok(Text::from("stale").encode_to_vec())
            })
            .is_err()
    );
    assert!(!encoded.load(Ordering::SeqCst));
    let replacement = runtime.operations.capture_callback(host.scope.as_ref().unwrap(), &contract).unwrap();
    let bytes = replacement.submit_with(NonZeroUsize::new(64).unwrap(), |_| Ok(Text::from("live").encode_to_vec())).unwrap().await.unwrap();
    assert_eq!(String::decode(&bytes).unwrap(), "live:1");
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn encoder_panic_releases() {
    let runtime = super::super::runtime_with_preparation(NonZeroUsize::new(128).unwrap()).await;
    let mut manager = PluginManager::new(candidate()).unwrap();
    let mut host = host(runtime.operations.clone());
    manager.initialize(&mut host).await.unwrap();
    let contract = WireDeclaration::of::<super::super::cases::Echo>().contract().unwrap();
    let target = runtime.operations.capture_callback(host.scope.as_ref().unwrap(), &contract).unwrap();
    assert!(matches!(
        target.submit_with(NonZeroUsize::new(64).unwrap(), |_| panic!("fixture encoder panicked")),
        Err(Error::OperationPanicked { .. })
    ));
    let bytes = target.submit_with(NonZeroUsize::new(64).unwrap(), |_| Ok(Text::from("released").encode_to_vec())).unwrap().await.unwrap();
    assert_eq!(String::decode(&bytes).unwrap(), "released:1");
    assert_eq!(host.owner.prepared.load(Ordering::SeqCst), 1);
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn preparation_credit_held() {
    let credit = Credit::start(false).await;
    let invocation = credit.target.submit_with(NonZeroUsize::new(64).unwrap(), |_| Ok(Text::from("block").encode_to_vec())).unwrap();
    credit.cleanup.started.notified().await;
    credit.assert_held();
    invocation.cancel();
    credit.cleanup.finalizing.notified().await;
    credit.assert_held();
    credit.cleanup.fail.store(true, Ordering::SeqCst);
    credit.cleanup.release.notify_one();
    assert!(matches!(invocation.await, Err(Error::OperationFinalization { .. })));
    credit.assert_held();
    credit.settle().await;
}

#[tokio::test]
async fn plugin_catalog_lifecycle() {
    let runtime = runtime().await;
    let mut manager = PluginManager::new(candidate()).unwrap();
    let mut host = host(runtime.operations.clone());
    manager.initialize(&mut host).await.unwrap();
    assert_eq!(runtime.operations.definitions().len(), 8);
    assert_eq!(runtime.operations.owner(Compute::ID).unwrap().as_str(), "fixture");
    assert!(matches!(runtime.operations.stage_plugin(host.scope.as_ref().unwrap(), |_| Ok(())), Err(Error::InvalidInput { .. })));
    assert_eq!(runtime.operations.call(request()).await.unwrap().value, 7);

    manager.shutdown(&mut host).await.unwrap();
    assert!(runtime.operations.definitions().is_empty());
    assert!(matches!(runtime.operations.stage_plugin(host.scope.as_ref().unwrap(), |_| Ok(())), Err(Error::Closing { .. })));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn staging_releases_names() {
    let runtime = runtime().await;
    let mut manager = PluginManager::new(candidate()).unwrap();
    let mut host = host(runtime.operations.clone());
    host.fail_start = true;
    assert!(manager.initialize(&mut host).await.is_err());
    assert!(runtime.operations.definitions().is_empty());
    assert_eq!(host.owner.prepared.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(runtime.operations.register_operation::<ComputeCall>("fixture", Lanes::Open, Arc::new(Owner::default())).is_ok());

    host.fail_start = false;
    assert!(manager.initialize(&mut host).await.is_err());
    // The second binding was never reserved when the complete candidate collided.
    assert!(runtime.operations.register_operation::<SlowCall>("fixture", Lanes::Open, Arc::new(Owner::default())).is_ok());
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn dropped_receipt_defers() {
    let running = Running::start().await;
    let runtime = &running.runtime;
    let mut manager = running.manager;
    let mut host = running.host;
    manager.shutdown(&mut host).await.unwrap();
    assert!(runtime.operations.definitions().is_empty());
    assert!(matches!(
        runtime.operations.register_operation::<ComputeCall>("fixture", Lanes::Open, Arc::new(Owner::default())),
        Err(Error::AlreadyRegistered { .. })
    ));

    running.gate.notify_one();
    assert_eq!(running.invocation.await.unwrap().value, 7);
    runtime.operations.register_operation::<ComputeCall>("fixture", Lanes::Open, Arc::new(Owner::default())).unwrap();
    assert_eq!(runtime.operations.owner(Compute::ID).unwrap().as_str(), "fixture");
    running.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn unregister_cancel_retained() {
    let running = Running::start().await;
    let runtime = &running.runtime;
    let mut manager = running.manager;
    let mut host = running.host;
    {
        // The gated invocation holds retirement, so the first poll cannot finish it.
        let mut unregister = std::pin::pin!(runtime.operations.unregister(Compute::ID));
        assert!(futures_util::poll!(&mut unregister).is_pending());
    }
    manager.shutdown(&mut host).await.unwrap();

    running.gate.notify_one();
    assert_eq!(running.invocation.await.unwrap().value, 7);
    assert!(matches!(
        runtime.operations.register_operation::<ComputeCall>("fixture", Lanes::Open, Arc::new(Owner::default())),
        Err(Error::AlreadyRegistered { .. })
    ));
    runtime.operations.unregister(Compute::ID).await.unwrap();
    runtime.operations.register_operation::<ComputeCall>("fixture", Lanes::Open, Arc::new(Owner::default())).unwrap();
    running.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn plugin_caller_authorization() {
    let runtime = runtime().await;
    let mut manager = PluginManager::new(with_clients(true)).unwrap();
    let mut host = host(runtime.operations.clone());
    host.call_during_setup = true;
    manager.initialize(&mut host).await.unwrap();
    let caller = runtime.operations.caller(&host.callers["client"]).unwrap();
    assert_eq!(caller.call(Shared(request())).await.unwrap().value, 7);
    // Parent is shared; its resource-owner context can call its private child.
    assert_eq!(caller.call(Parent(request())).await.unwrap().value, 7);
    let (requests, mut decoded) = harmos_runtime::operations::stream(harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(4096).unwrap(),
    })
    .unwrap();
    requests.send_with(4096, || Ok(Shared(request()))).await.unwrap();
    let charged = decoded.recv().await.unwrap();
    assert_eq!(caller.submit_charged::<Shared, _>(charged, ()).unwrap().await.unwrap().value, 7);
    drop(requests.reserve(4096).await.unwrap());
    #[cfg(feature = "grpc")]
    {
        let response = grpc_call::<super::super::cases::Echo>(harmos_runtime::grpc::server::Authority::Plugin(caller.clone())).await;
        use http_body_util::BodyExt as _;
        let body = response.into_body().collect().await.unwrap();
        assert_eq!(body.trailers().unwrap()["grpc-status"], "0");
        let bytes = body.to_bytes();
        assert_eq!(bytes[0], 0);
        assert_eq!(u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize, bytes.len() - 5);
        assert_eq!(<String as harmos_runtime::Protobuf>::decode(&bytes[5..]).unwrap(), "grpc:1");
        let response = grpc_call::<super::super::cases::PrivateEcho>(harmos_runtime::grpc::server::Authority::Plugin(caller.clone())).await;
        assert_eq!(response.headers()["grpc-status"], "7");
    }
    let prepared = host.owner.prepared.load(std::sync::atomic::Ordering::SeqCst);
    for source in ["client", "transitive", "unrelated"] {
        let source = runtime.operations.caller(&host.callers[source]).unwrap();
        assert!(matches!(source.submit(request()), Err(Error::OperationUnauthorized { .. })));
        let (output, input) = harmos_runtime::operations::stream::<u32>(harmos_runtime::operations::StreamSettings {
            items: NonZeroUsize::new(1).unwrap(),
            bytes: NonZeroUsize::new(4).unwrap(),
        })
        .unwrap();
        assert!(matches!(source.submit_endpoints::<Compute, _>(request(), input), Err(Error::OperationUnauthorized { .. })));
        assert!(output.is_closed());
        // Each Operation Charges Its Own Request Type.
        let (requests, mut decoded) = harmos_runtime::operations::stream(harmos_runtime::operations::StreamSettings {
            items: NonZeroUsize::new(1).unwrap(),
            bytes: NonZeroUsize::new(4096).unwrap(),
        })
        .unwrap();
        requests.send_with(4096, || Ok(request())).await.unwrap();
        let charged = decoded.recv().await.unwrap();
        assert!(matches!(source.submit_charged::<Compute, _>(charged, ()), Err(Error::OperationUnauthorized { .. })));
        drop(requests.reserve(4096).await.unwrap());
    }
    for source in ["transitive", "unrelated"] {
        let source = runtime.operations.caller(&host.callers[source]).unwrap();
        assert!(matches!(source.submit(Shared(request())), Err(Error::OperationUnauthorized { .. })));
    }
    assert_eq!(host.owner.prepared.load(std::sync::atomic::Ordering::SeqCst), prepared);
    assert_eq!(runtime.operations.caller(&host.callers["fixture"]).unwrap().call(request()).await.unwrap().value, 7);
    manager.shutdown(&mut host).await.unwrap();
    assert!(matches!(caller.submit(Shared(request())), Err(Error::OperationCaller { .. })));
    let (output, input) = harmos_runtime::operations::stream::<u32>(harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(4).unwrap(),
    })
    .unwrap();
    assert!(matches!(caller.submit_endpoints::<Shared, _>(Shared(request()), input), Err(Error::OperationCaller { .. })));
    assert!(output.is_closed());
    requests.send_with(4096, || Ok(Shared(request()))).await.unwrap();
    let charged = decoded.recv().await.unwrap();
    assert!(matches!(caller.submit_charged::<Shared, _>(charged, ()), Err(Error::OperationCaller { .. })));
    drop(requests.reserve(4096).await.unwrap());
    #[cfg(feature = "grpc")]
    {
        let response = grpc_call::<super::super::cases::Echo>(harmos_runtime::grpc::server::Authority::Plugin(caller.clone())).await;
        assert_eq!(response.headers()["grpc-status"], "16");
    }
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn operation_cache_partitioned() {
    let runtime = runtime().await;
    let mut manager = PluginManager::new(with_clients(true)).unwrap();
    let mut host = host(runtime.operations.clone());
    manager.initialize(&mut host).await.unwrap();
    let caller = runtime.operations.caller(&host.callers["client"]).unwrap();
    let keyed = || Shared(Request { keyed: true, ..request() });
    let host_response = runtime.operations.call(keyed()).await.unwrap();
    let first = caller.call(keyed()).await.unwrap();
    let again = caller.call(keyed()).await.unwrap();
    assert!(!host_response.ptr_eq(&first));
    assert!(first.ptr_eq(&again));
    assert_eq!(host.owner.runs.load(std::sync::atomic::Ordering::SeqCst), 2);

    // Only the client generation changes; the provider binding/cache stays live.
    let mut next = with_clients(true).definitions().cloned().collect::<Vec<_>>();
    next.iter_mut().find(|definition| definition.id == "client").unwrap().configuration = br#"{"revision":2}"#.to_vec();
    manager.reload(PluginDefinitions::new(next, []).unwrap(), &mut host).await.unwrap();
    assert!(matches!(caller.submit(keyed()), Err(Error::OperationCaller { .. })));
    let replacement = runtime.operations.caller(&host.callers["client"]).unwrap();
    let next = replacement.call(keyed()).await.unwrap();
    assert!(!first.ptr_eq(&next));
    assert!(host_response.ptr_eq(&runtime.operations.call(keyed()).await.unwrap()));
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn requirement_revokes_operation() {
    let runtime = runtime().await;
    let mut manager = PluginManager::new(with_clients(true)).unwrap();
    let mut host = host(runtime.operations.clone());
    manager.initialize(&mut host).await.unwrap();
    let old = runtime.operations.caller(&host.callers["client"]).unwrap();
    manager.reload(with_clients(false), &mut host).await.unwrap();
    assert!(matches!(old.submit(Shared(request())), Err(Error::OperationCaller { .. })));
    let replacement = runtime.operations.caller(&host.callers["client"]).unwrap();
    assert!(matches!(replacement.submit(Shared(request())), Err(Error::OperationUnauthorized { .. })));
    assert_eq!(runtime.operations.call(Shared(request())).await.unwrap().value, 7);
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn queued_operations_recheck() {
    let runtime = runtime().await;
    let mut manager = PluginManager::new(with_clients(true)).unwrap();
    let mut host = host(runtime.operations.clone());
    manager.initialize(&mut host).await.unwrap();
    let source = runtime.operations.caller(&host.callers["client"]).unwrap();
    let opened = Arc::new(tokio::sync::Notify::new());
    let gate = Arc::new(tokio::sync::Notify::new());
    let held = runtime.operations.submit(Shared(Request { gate: Some((opened.clone(), gate.clone())), ..request() })).unwrap();
    opened.notified().await;
    let queued = source.submit(Shared(request())).unwrap();
    manager.reload(with_clients(false), &mut host).await.unwrap();

    gate.notify_one();
    held.await.unwrap();
    assert!(matches!(queued.await, Err(Error::OperationCaller { .. })));
    assert_eq!(host.owner.runs.load(std::sync::atomic::Ordering::SeqCst), 1);
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn unmanaged_endpoint_refused() {
    let runtime = runtime().await;
    let router = Router::new(candidate().graph().clone());
    let endpoint = router.enter("fixture").unwrap();
    assert!(endpoint.scope().is_none());
    assert!(matches!(runtime.operations.caller(&endpoint.caller()), Err(Error::OperationCaller { .. })));
    drop(endpoint);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn encoded_calls_authorized() {
    use super::super::cases::{Echo, Nested, PrivateEcho};
    use harmos_runtime::Message;
    let runtime = runtime().await;
    let mut manager = PluginManager::new(with_clients(true)).unwrap();
    let mut host = host(runtime.operations.clone());
    manager.initialize(&mut host).await.unwrap();
    let client = runtime.operations.caller(&host.callers["client"]).unwrap();
    let owner = runtime.operations.caller(&host.callers["fixture"]).unwrap();
    assert_eq!(String::decode(&client.call_encoded(Echo::ID, Text::from("hello").encode_to_vec()).await.unwrap()).unwrap(), "hello:1");
    assert_eq!(String::decode(&owner.call_encoded(PrivateEcho::ID, Text::from("own").encode_to_vec()).await.unwrap()).unwrap(), "own:1");
    // The client's shared parent invokes its own private child as the bound owner.
    assert_eq!(&*client.call(Nested::from("nested")).await.unwrap(), "nested:1");
    let prepared = host.owner.prepared.load(std::sync::atomic::Ordering::SeqCst);
    // Malformed payloads must not replace authorization failures with codec errors.
    assert!(matches!(client.submit_encoded(PrivateEcho::ID, vec![0xff]), Err(Error::OperationUnauthorized { .. })));
    for source in ["transitive", "unrelated"] {
        let source = runtime.operations.caller(&host.callers[source]).unwrap();
        assert!(matches!(source.submit_encoded(Echo::ID, vec![0xff]), Err(Error::OperationUnauthorized { .. })));
    }
    assert_eq!(host.owner.prepared.load(std::sync::atomic::Ordering::SeqCst), prepared);
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn staging_refuses_metadata() {
    for change in 0..3 {
        let runtime = runtime().await;
        let mut definition = candidate().definitions().next().unwrap().clone();
        match change {
            0 => {
                definition.contributions.operations.pop();
            }
            1 => {
                definition.contributions.operations[0].policy.timeout += std::time::Duration::from_secs(1);
            }
            _ => {
                definition.contributions.operations[0].visibility = harmos_runtime::operations::Visibility::Shared;
            }
        }
        let mut manager = PluginManager::new(PluginDefinitions::new([definition], []).unwrap()).unwrap();
        let mut host = host(runtime.operations.clone());
        assert!(manager.initialize(&mut host).await.is_err());
        assert!(runtime.operations.definitions().is_empty());
        assert_eq!(host.owner.prepared.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(host.owner.runs.load(std::sync::atomic::Ordering::SeqCst), 0);
        manager.shutdown(&mut host).await.unwrap();
        runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn commit_requires_operation() {
    for missing in 0..3 {
        let runtime = runtime().await;
        let mut manager = PluginManager::new(candidate()).unwrap();
        let mut host = host(runtime.operations.clone());
        host.skip_operations = missing == 0;
        host.drop_before_commit = missing == 1;
        host.unregister_before_commit = missing == 2;
        let error = manager.initialize(&mut host).await.unwrap_err();
        let harmos_runtime::Error::PluginLifecycle { source } = &error else {
            panic!("plugin lifecycle failure is retained: {error:?}");
        };
        let harmos_runtime::plugin::PluginFailure::Start { source, .. } = source.as_ref() else {
            panic!("initialization failure is retained: {error:?}");
        };
        let harmos_runtime::Error::PluginLifecycle { source } = source else {
            panic!("typed routing cause is retained: {error:?}");
        };
        if missing == 1 {
            assert!(
                matches!(source.as_ref(), harmos_runtime::plugin::PluginFailure::Routing(harmos_runtime::plugin::DispatchError::Unavailable)),
                "dropping the original receipt closes staging: {error:?}"
            );
        } else {
            assert!(
                matches!(source.as_ref(), harmos_runtime::plugin::PluginFailure::Routing(
                harmos_runtime::plugin::DispatchError::IncompleteOperations { owner }
            ) if owner == "fixture"),
                "{error:?}"
            );
        }
        assert!(runtime.operations.definitions().is_empty());
        assert!(!host.scope.as_ref().unwrap().is_ready());
        manager.shutdown(&mut host).await.unwrap();
        runtime.stop().await.unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn preparation_credit_expired() {
    let credit = Credit::start(true).await;
    let invocation = credit.target.submit_with(NonZeroUsize::new(64).unwrap(), |_| Ok(Text::from("expire").encode_to_vec())).unwrap();
    credit.cleanup.finalizing.notified().await;
    assert_eq!(credit.cleanup.attempted.load(Ordering::SeqCst), 0);
    credit.assert_held();
    credit.cleanup.release.notify_one();
    credit.cleanup.failed.notified().await;
    credit.assert_held();
    let outcome = invocation.await;
    assert!(matches!(outcome,
        Err(Error::OperationFinalization { outcome: Some(ref error), .. })
            if matches!(error.as_ref(), Error::OperationRefused(harmos_runtime::Refusal::TimedOut))
    ));
    credit.settle().await;
}

#[tokio::test]
async fn provider_selection_reload() {
    let runtime = runtime().await;
    let mut host = SelectedHost { operations: runtime.operations.clone(), scopes: BTreeMap::new(), callers: BTreeMap::new() };
    let mut manager = PluginManager::new(selected_candidate("first", true)).unwrap();
    manager.initialize(&mut host).await.unwrap();
    let contract = WireDeclaration::of::<Selected>().contract().unwrap();
    let first = runtime.operations.capture_callback(&host.scopes["first"], &contract).unwrap();
    let second = runtime.operations.capture_callback(&host.scopes["second"], &contract).unwrap();
    let old_client = runtime.operations.caller(&host.callers["client"]).unwrap();
    assert_eq!(old_client.call(Selected::from("before")).await.unwrap().as_str(), "first:before");
    assert!(matches!(
        runtime.operations.call(Selected::from("ambiguous")).await,
        Err(Error::InvalidInput { field: "operation provider", .. })
    ));
    let first_generation = host.scopes["first"].generation();
    let second_generation = host.scopes["second"].generation();

    manager.reload(selected_candidate("second", true), &mut host).await.unwrap();
    assert!(matches!(old_client.submit(Selected::from("stale")), Err(Error::OperationCaller { .. })));
    assert_eq!(host.scopes["first"].generation(), first_generation);
    assert_eq!(host.scopes["second"].generation(), second_generation);
    let client = runtime.operations.caller(&host.callers["client"]).unwrap();
    assert_eq!(client.call(Selected::from("after")).await.unwrap().as_str(), "second:after");
    let first_payload =
        first.submit_with(NonZeroUsize::new(128).unwrap(), |_| Ok(Text::from("callback").encode_to_vec())).unwrap().await.unwrap();
    assert_eq!(String::decode(&first_payload).unwrap(), "first:callback");

    manager.reload(selected_candidate("second", false), &mut host).await.unwrap();
    assert!(matches!(
        first.submit_with(NonZeroUsize::new(128).unwrap(), |_| Ok(Text::from("retired").encode_to_vec())),
        Err(Error::Closing { .. })
    ));
    let second_payload =
        second.submit_with(NonZeroUsize::new(128).unwrap(), |_| Ok(Text::from("surviving").encode_to_vec())).unwrap().await.unwrap();
    assert_eq!(String::decode(&second_payload).unwrap(), "second:surviving");
    assert_eq!(host.scopes["second"].generation(), second_generation);
    assert_eq!(client.call(Selected::from("unchanged")).await.unwrap().as_str(), "second:unchanged");
    manager.shutdown(&mut host).await.unwrap();
    assert!(matches!(
        second.submit_with(NonZeroUsize::new(128).unwrap(), |_| Ok(Text::from("closed").encode_to_vec())),
        Err(Error::Closing { .. })
    ));
    drop((first, second, client, old_client, manager, host));
    runtime.stop().await.unwrap();
}

#[cfg(feature = "grpc")]
async fn grpc_call<O>(authority: harmos_runtime::grpc::server::Authority) -> tonic::codegen::http::Response<tonic::body::Body>
where
    O: OperationDeclaration<Response = String>,
{
    use tonic::codec::Codec as _;
    let capacity = NonZeroUsize::new(4096).unwrap();
    let decode = harmos_runtime::grpc::DecodeSettings { message_bytes: capacity, retained_bytes: capacity };
    let encode = harmos_runtime::grpc::EncodeSettings { message_bytes: capacity, buffer_bytes: capacity };
    let channel = harmos_runtime::operations::StreamSettings { items: NonZeroUsize::new(1).unwrap(), bytes: capacity };
    let settings = harmos_runtime::grpc::server::Settings {
        calls: NonZeroUsize::new(8).unwrap(),
        decode,
        encode,
        input: channel,
        output: channel,
        opening_timeout: std::time::Duration::from_secs(1),
    };
    let mut codec = harmos_runtime::grpc::Codec::<Text, harmos_runtime::operations::Response<Text>>::new(decode, encode).unwrap();
    let message = futures_util::stream::iter([Ok(harmos_runtime::operations::Response::new(Text::from("grpc")))]);
    let body = tonic::codec::EncodeBody::new_client(codec.encoder(), message, None, Some(4096));
    let request =
        tonic::codegen::http::Request::builder().header("content-type", "application/grpc").body(tonic::body::Body::new(body)).unwrap();
    harmos_runtime::grpc::server::unary::<O>(authority, settings, request).await
}

/* === Tests === */

