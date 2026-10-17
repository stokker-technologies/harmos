/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Registration Catalogs And Initialization Gate Admission Of One Sidecar Generation

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::Mutex;

// Third Party Imports

// Local Imports
use super::*;
use crate::witness::Witness;
use harmos::sidecar::{Registrar, Registration, RegistrationError};

/* === Constants === */
const NAME: &str = "admission-fixture";

// `register` Takes No Configuration, So The Story's Witness Is Reached Here.
static CURRENT: Mutex<Option<Arc<Witness>>> = Mutex::new(None);

// Stories Sharing `CURRENT` Run One At A Time Under Any Runner.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */
type Source = harmos_sidecar::PluginNative<Program>;

/* === Structs === */
// Witnesses Registration And Initialization Separately.
struct Program {
    marks: Arc<Witness>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for Program {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos::plugin(description = "Witnesses Registration And Initialization Separately.",
    id = "admission-fixture",
    provides = (),
    requires = (),
)]
impl harmos::Plugin for Program {
    fn register(registrar: &mut Registrar) -> Result<(), RegistrationError> {
        let marks = current();
        let catalog = if marks.seen("catalog-changed") {
            "changed"
        } else {
            "first"
        };
        registrar.register("fixture", 1, &catalog)?;
        registrar.register("unknown", 2, &vec![3, 4])?;
        marks.mark("registered");
        Ok(())
    }

    type Context = Supervisor;
    type Config = ();
    type Error = Refusal;
    type Resources = ();

    fn prepare(_: ()) -> Result<Self, Refusal> {
        Ok(Self { marks: current() })
    }

    async fn initialize(&mut self, host: &Supervisor) -> Result<(), Refusal> {
        self.marks.mark("initialized");
        if self.marks.seen("refuse") {
            return Err(Refusal::of("initialization refused"));
        }
        self.marks.marked("release").await;
        let marks = Arc::clone(&self.marks);
        let host = host.clone();
        tokio::spawn(async move {
            marks.marked("crash").await;
            host.abandon();
        });
        Ok(())
    }

    fn resources(&self) -> Arc<()> {
        Arc::new(())
    }

    fn bind_operations(&self, _: &mut WireTable<Supervisor>) -> Result<(), harmos::operations::ServingError> {
        Ok(())
    }

    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn current() -> Arc<Witness> {
    CURRENT
        .lock()
        .expect("the current witness is never poisoned")
        .clone()
        .expect("an admission story installs its witness")
}

fn install() -> Arc<Witness> {
    let marks = Witness::open();
    *CURRENT.lock().expect("the current witness is never poisoned") = Some(Arc::clone(&marks));
    marks
}

fn expected() -> Vec<Registration> {
    let mut registrar = Registrar::new();
    registrar.register("fixture", 1, &"first").unwrap();
    registrar.register("unknown", 2, &vec![3, 4]).unwrap();
    let mut entries = registrar.finish();
    entries.reverse();
    entries
}

async fn native(expected: &[Registration]) -> operations::Managed<Source> {
    let source = Source::new(NonZeroUsize::new(1).unwrap()).unwrap();
    let definitions = operations::inspected(&source, serde_json::to_vec(&()).unwrap());
    let resident = Supervised::new(source, notices as fn(&Service<Bank>, Notice))
        .expected_registrations(expected);
    let runtime = assembly()
        .register(operations::Lanes::catalog())
        .operation_settings(harmos::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    operations::Managed::from_runtime(
        runtime,
        definitions,
        resident,
        operations::Lanes::NativeOpen,
        NonZeroUsize::new(1).unwrap(),
        harmos::services::POLICY,
    )
}

fn notices(_: &Service<Bank>, _: Notice) {}

#[tokio::test]
async fn catalog_drift_refused() {
    let _serial = SERIAL.lock().await;
    let drifts: [fn(&mut Vec<Registration>); 4] = [
        |entries| entries[0].payload = b"[]".to_vec(),
        |entries| entries[0].schema += 1,
        |entries| {
            entries.pop();
        },
        |entries| entries.push(entries[0].clone()),
    ];
    for drift in drifts {
        let marks = install();
        let mut entries = expected();
        drift(&mut entries);
        let mut fixture = native(&entries).await;
        let ready = fixture.resident().ready();
        let registrations = fixture.resident().registrations();

        // The Frozen Catalog Differs, So Initialization Never Starts
        assert!(fixture.initialize().await.is_err());
        let runtime = &fixture.runtime;
        let status = service_status(runtime.operations.watch_residents(), NAME, |value| {
            matches!(value, ServiceStatus::Inactive { .. })
        })
        .await;
        assert!(
            matches!(status, ServiceStatus::Inactive { cause: ServiceExit::Application(ref reason), .. } if reason.contains("expected catalog")),
            "{status:?}"
        );
        assert!(marks.seen("registered"));
        assert!(!marks.seen("initialized"));
        assert!(!*ready.borrow());
        assert!(registrations.borrow().is_none());
        assert!(matches!(
            runtime.stop().await,
            Err(harmos::Error::TaskFailed {
                domain: "service",
                ..
            })
        ));
    }
}

#[tokio::test]
async fn readiness_requires_init() {
    let _serial = SERIAL.lock().await;
    let marks = install();
    let mut fixture = native(&expected()).await;
    let calls = fixture.resident().clone();
    let ready = calls.ready();
    let mut registrations = calls.registrations();
    let services = fixture.runtime.operations.clone();
    assert!(!*ready.borrow());

    // Registration Completes While Initialization Is Still Held
    {
        let initialization = fixture.initialize();
        tokio::pin!(initialization);
        tokio::select! {
            result = &mut initialization => panic!("SDK initialization passed the unreleased gate: {result:?}"),
            result = registrations.wait_for(Option::is_some) => {
                result.unwrap();
            }
        }
        assert!(!*ready.borrow());
        assert_eq!(
            services
                .resident(&harmos::operations::resident::Key::root(NAME))
                .and_then(|item| item.health),
            Some(ServiceStatus::Starting)
        );
        assert!(matches!(calls.operations(), Err(SidecarError::Unavailable)));
        marks.mark("release");
        initialization.await.unwrap();
    }

    // Readiness Follows Initialization And Ends With Shutdown
    publications::ready(ready.clone()).await;
    service_status(services.watch_residents(), NAME, |value| {
        matches!(value, ServiceStatus::Running { .. })
    })
    .await;
    assert_eq!(marks.count("initialized"), 1);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
    assert!(!*ready.borrow());
    assert!(registrations.borrow().is_none());
}

#[tokio::test]
async fn refused_init_unready() {
    let _serial = SERIAL.lock().await;
    let marks = install();
    marks.mark("refuse");
    let mut fixture = native(&expected()).await;
    let ready = fixture.resident().ready();

    let failure = fixture
        .initialize()
        .await
        .expect_err("the actual SDK initialization refuses");

    let cause = failure
        .failure()
        .expect("the original authored refusal crosses as a structured failure");
    assert!(
        cause.cause().contains("initialization refused"),
        "{cause:?}"
    );
    assert!(marks.seen("initialized"));
    assert!(!*ready.borrow());
    assert_eq!(
        fixture.status(NAME).unwrap().state,
        harmos::plugin::PluginState::Failed
    );
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn restart_closes_admission() {
    let _serial = SERIAL.lock().await;
    let marks = install();
    marks.mark("release");
    let policy = OperationPolicy {
        restart: RestartPolicy::OnTransient {
            credit: 1,
            sustained_health: Duration::from_secs(30),
            backoff: harmos::operations::Backoff::Exponential {
                initial: Duration::from_millis(1),
                maximum: Duration::from_millis(2),
                jitter: harmos::operations::Jitter::Deterministic,
            },
        },
        ..harmos::services::POLICY
    };
    let source = Source::new(NonZeroUsize::new(1).unwrap()).unwrap();
    let definition = operations::definition(&source, serde_json::to_vec(&()).unwrap());
    let calls = Supervised::new(source, notices as fn(&Service<Bank>, Notice))
        .expected_registrations(&expected());
    let ready = calls.ready();
    let registrations = calls.registrations();
    let runtime = assembly()
        .operation_settings(harmos::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let resident = calls.clone();
    runtime
        .operations
        .register_service(
            harmos::services::ServiceDefinition {
                id: NAME.into(),
                policy,
            },
            move |_: Arc<()>| {
                harmos::sidecar::Managed::new(
                    resident.clone(),
                    definition.clone(),
                    HostLanes::Open,
                    NonZeroUsize::new(1).unwrap(),
                    NonZeroUsize::new(4096).unwrap(),
                )
                .unwrap()
            },
        )
        .unwrap();
    let _lease = runtime.operations.submit_resident(NAME).await.unwrap();
    service_status(runtime.operations.watch_residents(), NAME, |status| {
        matches!(status, ServiceStatus::Running { .. })
    })
    .await;

    // The Restarted Generation Registers A Changed Catalog And Is Refused
    marks.mark("catalog-changed");
    marks.mark("crash");
    service_status(runtime.operations.watch_residents(), NAME, |value| {
        matches!(value, ServiceStatus::Inactive { .. })
    })
    .await;
    assert!(!*ready.borrow());
    assert!(registrations.borrow().is_none());
    assert!(matches!(calls.operations(), Err(SidecarError::Unavailable)));
    assert_eq!(marks.count("initialized"), 1);
    assert_eq!(marks.count("registered"), 2);
    assert!(matches!(
        runtime.stop().await,
        Err(harmos::Error::TaskFailed {
            domain: "service",
            ..
        })
    ));
}

/* === Tests === */
