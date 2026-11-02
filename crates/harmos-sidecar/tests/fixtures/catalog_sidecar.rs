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

// Third Party Imports

// Local Imports
use harmos_operations::lifecycle;
use harmos_operations::lifecycle::Policy;
use harmos_sdk::operations::{OperationPolicy, RetryPolicy};
use harmos_sdk::plugin::Plugin;
use harmos_sidecar::{Refusal, Registrar, RegistrationError, Supervisor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Catalog;

#[harmos_sidecar::operation(
    harmos_path = ::harmos_sidecar,
    id = "catalog.read",
    response = (),
    policy = Catalog::INITIALIZATION_POLICY,
    visibility = Public, grpc(package = "fixture.v1", service = "Native", method = "Read"),
)]
#[derive(Clone, PartialEq)]
struct ReadOperation {}

/* === Enums === */

#[harmos_macros::sidecar_catalog(harmos_path = ::harmos_sidecar)]
enum Readings {
    Read(ReadOperation),
}

/* === Traits === */

/* === Implementations === */

impl harmos_sdk::Operation for ReadOperation {
    type Error = Refusal;

    async fn run(self, _: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::Response<()>, Refusal> {
        panic!("catalog export must never execute an operation")
    }
}

impl lifecycle::Policy for Catalog {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: std::time::Duration::from_secs(120),
        attempt_timeout: std::time::Duration::from_secs(120),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_sdk::Lifecycle for Catalog {
    type Config = ();
    type Error = Refusal;
    type Context = Supervisor;

    async fn prepare(_: (), _setup: &mut harmos_sdk::Setup<Self>) -> Result<Self, Refusal> {
        panic!("export must never prepare an authored instance")
    }

    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        panic!("export must never initialize resources")
    }

    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        panic!("export must never enter domain shutdown")
    }

    async fn cleanup(&mut self) -> Result<(), Refusal> {
        panic!("export must never acquire a cleanup owner")
    }
}

#[harmos_macros::plugin(
    harmos_path = ::harmos_sidecar,
    id = "catalog_fixture",
    description = "Exports a catalog and never runs.",
    requires(ReadOperation),
)]
impl Plugin for Catalog {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[Readings::CATALOG];

    fn register(registrar: &mut Registrar) -> Result<(), RegistrationError> {
        registrar.register("zeta.catalog", 2, &serde_json::json!({"suite": "probe"}))?;
        registrar.register("alpha.catalog", 1, &vec![1, 2])
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--environment-probe") {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::{OsStrExt, OsStringExt};
            use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;

            let program_data = std::env::var_os("ProgramData").expect("the parent provides ProgramData");
            let source = program_data.encode_wide().chain(Some(0)).collect::<Vec<_>>();
            // SAFETY: The Source Is NUL-Terminated; A Null Destination With Zero Size Queries The Required Capacity.
            #[allow(unsafe_code)]
            let capacity = unsafe { ExpandEnvironmentStringsW(source.as_ptr(), std::ptr::null_mut(), 0) };
            if capacity == 0 {
                return Err(std::io::Error::last_os_error());
            }
            let mut destination = vec![0_u16; capacity as usize];
            // SAFETY: Both Buffers Are Valid For The Synchronous Call And The Destination Has The Queried Capacity.
            #[allow(unsafe_code)]
            let length = unsafe { ExpandEnvironmentStringsW(source.as_ptr(), destination.as_mut_ptr(), capacity) };
            if length == 0 || length > capacity {
                return Err(std::io::Error::other("ProgramData expansion failed"));
            }
            let expanded = std::ffi::OsString::from_wide(&destination[..length as usize - 1]);
            std::fs::create_dir_all(std::path::Path::new(&expanded).join("harmos-probe"))?;
        }
        let environment = std::env::vars().collect::<std::collections::BTreeMap<_, _>>();
        serde_json::to_writer(std::io::stdout(), &environment)?;
        return Ok(());
    }
    harmos_sidecar::run_plugin::<Catalog>(std::num::NonZeroUsize::new(1).unwrap()).await
}

/* === Tests === */
