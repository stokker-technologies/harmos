/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Before Full Startup

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use crate::storage::{Backend, RecoveryBackend, StorageManager, config, history, record, schema};
use harmos_runtime::backend::StorageBackends;
use harmos_runtime::catalog;
use harmos_runtime::facade::storage::{CapabilitiesRequest, InfoRequest, MutateRequest, MutationId, MutationItem};
use harmos_runtime::operations::{CallContext, Lane, Registration};
use harmos_runtime::runtime::{BootstrapStatus, StorageFailure};
use harmos_runtime::state::Never;
use harmos_runtime::{Application, Builder, Identifier, Lifecycle, Setup, Startup, StartupError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Minimal;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Lifecycle for Minimal {
    type Config = ();
    type Error = Infallible;
    type Context = Startup<Self>;

    async fn prepare(_: (), _: &mut Setup<Self>) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl Application for Minimal {
    type State = ();
    type Transaction = Never;
    type Record = ();
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn storage_only_runtime_admits_operations() {
    // Assemble Storage Before Starting the Application
    let primary = Arc::new(Backend::new(history()));
    let recovery = Arc::new(RecoveryBackend::new(history()));
    let manager = StorageManager::new(
        StorageBackends {
            primary: primary.clone(),
            state: recovery.clone(),
        },
        config(),
    )
    .unwrap();
    let mut builder = Builder::<Minimal>::new(());
    builder.storage(&manager);
    let registration = Registration::catalog(catalog::storage_catalog(), Lane::default()).unwrap();
    builder.with_registration(registration);
    let minimal = builder.storage_only().await.unwrap();
    let call = minimal.for_call(&CallContext::default());
    let capabilities = call.storage().capabilities(CapabilitiesRequest {}).await.unwrap();
    let mutation = MutationId {
        window: capabilities.mutations.unwrap().id,
        key: Identifier::new("before-start").unwrap(),
    };
    let item = MutationItem::Append(record(&schema(), &Identifier::new("bootstrap").unwrap(), &[1, 2, 3]));
    let request = MutateRequest {
        mutation: Some(mutation),
        items: vec![item],
    };
    call.storage().mutate(request).await.unwrap();

    // Drain and Close Before Full Assembly Reopens the Registered Backends
    minimal.close().await.unwrap();
    assert!(!primary.control.open.load(Ordering::SeqCst));
    assert!(!recovery.control.open.load(Ordering::SeqCst));
    assert!(call.storage().info(InfoRequest {}).await.is_err());
    drop(minimal);
    let runtime = builder.start().await.unwrap();
    assert!(primary.control.open.load(Ordering::SeqCst));
    assert!(recovery.control.open.load(Ordering::SeqCst));
    assert_eq!(primary.records.lock().unwrap().len(), 1);
    let call = runtime.for_call(&CallContext::default());
    assert_eq!(call.storage().info(InfoRequest {}).await.unwrap().saved.unwrap().sequence, 1);
    assert_eq!(primary.control.closes.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
    assert_eq!(primary.control.closes.load(Ordering::SeqCst), 2);
    assert_eq!(recovery.control.closes.load(Ordering::SeqCst), 2);

    // Retain Failed Cleanup and Refuse Reopening Until It Settles
    let manager = StorageManager::new(
        StorageBackends {
            primary: primary.clone(),
            state: recovery.clone(),
        },
        config(),
    )
    .unwrap();
    let mut builder = Builder::<Minimal>::new(());
    builder.storage(&manager);
    let session = builder.storage_only().await.unwrap();
    primary.control.fail_close.store(true, Ordering::SeqCst);
    assert!(session.close().await.is_err());
    drop(session);
    let cleanup = builder.storage_cleanup().unwrap();
    assert_eq!(cleanup.status(), BootstrapStatus::Failed);
    assert!(builder.storage_only().await.is_err());
    let rejected = match builder.start().await {
        Ok(_) => panic!("full startup must wait for storage cleanup"),
        Err(error) => error,
    };
    let StartupError::Runtime { source } = rejected else { panic!("missing cleanup failure") };
    let failure = source.downcast::<StorageFailure>().expect("startup retains the storage cleanup receipt");
    primary.control.fail_close.store(false, Ordering::SeqCst);
    failure.cleanup.retry().await.unwrap();
    assert!(!primary.control.open.load(Ordering::SeqCst));
    let mut builder = Builder::<Minimal>::new(());
    builder.storage(&manager);

    // Cancel Opening at an Explicit Backend Boundary
    primary.control.block_open.store(true, Ordering::SeqCst);
    {
        let opening = builder.storage_only();
        tokio::pin!(opening);
        tokio::select! {
            result = &mut opening => panic!("opening must wait for the backend: {}", result.is_ok()),
            _ = primary.control.open_entered.notified() => {}
        }
    }
    let cleanup = builder.storage_cleanup().unwrap();
    cleanup.wait().await.unwrap();
    assert!(!primary.control.open.load(Ordering::SeqCst));
    primary.control.block_open.store(false, Ordering::SeqCst);

    // Complete an Offline-Only Session Without Launching an Application
    let session = builder.storage_only().await.unwrap();
    let call = session.for_call(&CallContext::default());
    let info = call.storage().info(InfoRequest {}).await.unwrap();
    assert_eq!(info.saved.unwrap().sequence, 1);
    session.close().await.unwrap();
    drop(session);
    assert_eq!(builder.storage_cleanup().unwrap().status(), BootstrapStatus::Settled);
    assert!(!primary.control.open.load(Ordering::SeqCst));
}
