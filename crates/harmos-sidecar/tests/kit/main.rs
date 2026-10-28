/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

// Exercise The Original Helper Without Selecting The Authoring Kit.
#[path = "../../src/kit/task.rs"]
mod observer;
#[path = "../../../harmos-runtime/tests/support/fixtures.rs"]
mod prepared_fixtures;
#[path = "../kit/task.rs"]
mod task;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_sdk::operation(harmos_path = ::harmos_sdk, id = "storage.read_file", policy = harmos_sdk::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    retries: harmos_sdk::RetryPolicy::Never,
})]
struct OwnerRead;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_sdk::Operation for OwnerRead {
    type Error = std::convert::Infallible;

    async fn run(self, _: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::Response<()>, Self::Error> {
        Ok(harmos_sdk::Response::new(()))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[test]
fn owner_registration_records() {
    use harmos_domain::state::{FactSource, Records, SystemFact};

    let cases: Vec<(String, bool)> =
        serde_json::from_str(include_str!("../../../harmos-domain/tests/fixtures/owner-identities.json")).unwrap();
    for (owner, valid) in cases {
        let table = harmos_sidecar::operations::WireTable::new(&owner);
        assert_eq!(table.is_ok(), valid, "registration {owner:?}");
        if let Ok(mut table) = table {
            let batch = harmos_sidecar::RegistrationBatch::new(harmos_sidecar::Dependencies::new()).operation::<OwnerRead>();
            table.register(batch).unwrap();
            table.validate(&[<OwnerRead as harmos_sidecar::OperationDeclaration>::definition()]).unwrap();
        }
        let fact = SystemFact {
            source: FactSource::Sidecar(owner.clone()),
            name: "result/sample".to_owned(),
            version: 1,
            payload: vec![0, 255, 17],
        };
        let mut records = Records::default();
        assert_eq!(records.push_system(fact).is_ok(), valid, "records {owner:?}");
    }
}

#[test]
fn inert_native_metadata() {
    let root = tempfile::tempdir().unwrap();
    let artifact = root.path().join("plugin.data");
    // A Non-Executable Copy Suffices; The Plugin Lifecycle Panics If Entered.
    std::fs::write(
        &artifact,
        std::fs::read(prepared_fixtures::binary(
            "harmos-sidecar-catalog-fixture",
            option_env!("CARGO_BIN_EXE_harmos-sidecar-catalog-fixture"),
        ))
        .unwrap(),
    )
    .unwrap();
    let bytes = std::fs::read(&artifact).unwrap();
    let declaration = harmos_sidecar::inventory::Declaration::inspect(&bytes, harmos_sidecar::inventory::Facility::Sidecar).unwrap();
    assert!(!declaration.contracts.provides.is_empty());
}
