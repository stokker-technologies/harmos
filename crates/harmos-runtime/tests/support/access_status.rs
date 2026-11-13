/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![allow(dead_code, reason = "shared access/status fixture consumers use different slices")]

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

// Third Party Imports
use harmos_protocol::v1::{self, access};
use harmos_runtime::{
    Runtime,
    grpc::access::{CredentialStore, Credentials, Pending},
    services::status::{Contribution, Entry, Status, StatusSettings},
};
use tonic::{
    Request,
    transport::{Certificate, Channel, ClientTlsConfig, Endpoint},
};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/* === Structs === */

pub struct Model;
#[derive(Default)]
pub struct Store {
    pub value: Mutex<Option<Credentials>>,
    pub unavailable: AtomicBool,
    pub fail_commit: AtomicBool,
}
pub struct Host {
    pub runtime: Runtime<Model>,
    pub status: Arc<Status>,
    pub store: Arc<Store>,
    pub channel: Channel,
    pub management: harmos_runtime::grpc::Attached,
}
pub struct Facts {
    pub component: Mutex<v1::HealthComponent>,
    pub text: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;
    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }
}
impl harmos_runtime::Application for Model {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
}
impl CredentialStore for Store {
    fn load(&self) -> Pending<'_, Option<Credentials>> {
        Box::pin(async move {
            if self.unavailable.load(Ordering::Acquire) {
                return Err(std::io::Error::other("unavailable").into());
            }
            Ok(self.value.lock().unwrap().clone())
        })
    }
    fn commit(&self, expected: u64, value: Credentials) -> Pending<'_, ()> {
        Box::pin(async move {
            if self.unavailable.load(Ordering::Acquire) || self.fail_commit.load(Ordering::Acquire) {
                return Err(std::io::Error::other("unavailable").into());
            }
            let mut stored = self.value.lock().unwrap();
            if stored.as_ref().map_or(0, |v| v.revision) != expected {
                return Err(std::io::Error::other("revision conflict").into());
            }
            *stored = Some(value);
            Ok(())
        })
    }
}
impl Host {
    pub async fn start() -> Result<Self> {
        Self::with_store(Arc::new(Store::default())).await
    }
    pub async fn with_store(store: Arc<Store>) -> Result<Self> {
        let (clock, _) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
        let runtime = Runtime::<Model>::launch_with((), (), |setup| {
            setup.settings().clock(clock);
            setup.settings().operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(4 * 1024 * 1024).unwrap()),
                ..Default::default()
            });
        })
        .await?;
        let status = runtime.services().configure_status(StatusSettings {
            application_name: "Example Host".into(),
            application_version: "1.2.3".into(),
            host_id: "stable-example-host".into(),
            history_items: nz(16),
            history_bytes: nz(64 * 1024),
            read_leases: nz(32),
            archive_bytes: nz(256 * 1024),
            range_bytes: nz(4096),
            read_timeout: std::time::Duration::from_secs(30),
            redaction_bytes: nz(4096),
            contributions: nz(8),
        })?;
        runtime
            .grpc()
            .configure_access(
                harmos_runtime::grpc::access::AccessSettings {
                    listener: access::ListenerValue {
                        enabled: false,
                        endpoint: "127.0.0.1:0".into(),
                        certificate_names: vec!["localhost".into(), "127.0.0.1".into()],
                    },
                    transport: harmos_runtime::grpc::Settings::default(),
                    handshake_timeout: std::time::Duration::from_secs(5),
                    pairings: nz(8),
                    label_bytes: nz(128),
                },
                store.clone(),
            )
            .await?;
        let management = runtime.grpc().attach(harmos_runtime::grpc::Settings::default()).await?;
        let channel = Endpoint::from_shared(format!("http://{}", management.local_addr()))?.connect().await?;
        Ok(Self {
            runtime,
            status,
            store,
            channel,
            management,
        })
    }
    pub async fn close(self) -> Result {
        drop(self.channel);
        self.management.detach().await?;
        self.runtime.shutdown().await?;
        Ok(())
    }
    pub async fn enable(&self) -> Result<access::ListenerSetResponse> {
        let mut client = access::listener_client::ListenerClient::new(self.channel.clone());
        let current = client.get(access::ListenerGetRequest {}).await?.into_inner();
        let mut value = current.value.unwrap();
        value.enabled = true;
        Ok(client
            .set(access::ListenerSetRequest {
                value: Some(value),
                expected_revision: current.revision,
            })
            .await?
            .into_inner())
    }
}
impl Facts {
    pub fn new(text: &str) -> Self {
        Self {
            component: Mutex::new(v1::HealthComponent {
                name: "host.component".into(),
                state: v1::ComponentState::Healthy as i32,
                reason: "Ready".into(),
            }),
            text: text.into(),
        }
    }
}
impl Contribution for Facts {
    fn health(&self) -> std::result::Result<Vec<v1::HealthComponent>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![self.component.lock().unwrap().clone()])
    }
    fn diagnostics(&self) -> std::result::Result<Vec<Entry>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![Entry {
            path: "host/component.txt".into(),
            text: self.text.clone(),
        }])
    }
}

/* === Public Functions === */

pub fn assert_action(status: tonic::Status, expected: &str) {
    let failure = harmos_runtime::Failure::decode(status.details()).expect("operation failure details");
    let wire = harmos_protocol::v1::Failure::from(&failure);
    assert_eq!(wire.action, expected);
    assert!(wire.message.contains(expected));
}

pub fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
pub fn page(size: u32, token: String) -> v1::PageRequest {
    v1::PageRequest {
        page_size: size,
        max_bytes: 4096,
        page_token: token,
    }
}
pub fn authorized<T>(value: T, credential: &str) -> Request<T> {
    let mut request = Request::new(value);
    request.metadata_mut().insert("authorization", format!("Bearer {credential}").parse().unwrap());
    request
}
pub async fn paired(pairing: &access::PairingsCreateResponse) -> Result<Channel> {
    Ok(Endpoint::from_shared(pairing.endpoint.clone())?
        .tls_config(ClientTlsConfig::new().ca_certificate(Certificate::from_pem(&pairing.pinned_certificate_pem)))?
        .connect()
        .await?)
}

/* === Private Functions === */

/* === Tests === */
