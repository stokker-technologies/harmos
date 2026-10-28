/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise The Original Helper Without Selecting The Authoring Kit.
#[path = "../../src/kit/task.rs"]
mod observer;
#[path = "../../../harmos-runtime/tests/support/fixtures.rs"]
mod prepared_fixtures;
#[path = "../kit/task.rs"]
mod task;

#[harmos_sdk::operation(harmos_path = ::harmos_sdk, id = "storage.read_file", policy = harmos_sdk::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    retries: harmos_sdk::RetryPolicy::Never,
})]
struct OwnerRead;

impl harmos_sdk::Operation for OwnerRead {
    type Error = std::convert::Infallible;

    async fn run(self, _: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::Response<()>, Self::Error> {
        Ok(harmos_sdk::Response::new(()))
    }
}

fn validate_table(mut table: harmos_sdk::__private::WireTable) {
    let batch = harmos_sdk::__private::RegistrationBatch::new().operation::<OwnerRead>();
    table.register(batch).unwrap();
    table.validate_binding().unwrap();
    table.validate(&[<OwnerRead as harmos_sidecar::OperationDeclaration>::definition()]).unwrap();
}

#[test]
fn owner_registration_records() {
    use harmos_domain::state::{FactSource, Records, SystemFact};

    let cases: Vec<(String, bool)> =
        serde_json::from_str(include_str!("../../../harmos-domain/tests/fixtures/owner-identities.json")).unwrap();
    for (owner, valid) in cases {
        let table = harmos_sdk::__private::WireTable::new(&owner);
        assert_eq!(table.is_ok(), valid, "registration {owner:?}");
        let _registered = table.map(validate_table);
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
    let fixture = prepared_fixtures::binary("harmos-sidecar-catalog-fixture", option_env!("CARGO_BIN_EXE_harmos-sidecar-catalog-fixture"));
    let executable = std::fs::read(fixture).unwrap();
    std::fs::write(&artifact, executable).unwrap();
    let bytes = std::fs::read(&artifact).unwrap();
    let declaration = harmos_sidecar::inventory::Declaration::inspect(&bytes, harmos_sidecar::inventory::Facility::Sidecar).unwrap();
    assert!(!declaration.contracts.provides.is_empty());
}
