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

#[harmos_operations::execution::operation(harmos_path = ::harmos_operations::execution, id = "storage.read_file", policy = harmos_operations::execution::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_operations::execution::RetryPolicy::Never,
})]
struct OwnerRead;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_operations::execution::Operation for OwnerRead {
    type Error = std::convert::Infallible;

    async fn run(
        self, _: harmos_operations::execution::Context<'_, Self>,
    ) -> Result<harmos_operations::execution::Response<()>, Self::Error> {
        Ok(harmos_operations::execution::Response::new(()))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn validate_table(mut table: harmos_operations::execution::__private::WireTable) {
    let batch = harmos_operations::execution::__private::RegistrationBatch::new().operation::<OwnerRead>();
    table.register(batch).unwrap();
    table.validate_binding().unwrap();
    table.validate(&[<OwnerRead as harmos_sidecar::OperationDeclaration>::definition()]).unwrap();
}

/* === Tests === */

#[test]
fn owner_registration_records() {
    use harmos_domain::state::{FactSource, Records, SystemFact};

    let cases: Vec<(String, bool)> =
        serde_json::from_str(include_str!("../../../harmos-domain/tests/fixtures/owner-identities.json")).unwrap();
    for (owner, valid) in cases {
        let table = harmos_operations::execution::__private::WireTable::new(&owner);
        assert_eq!(table.is_ok(), valid, "registration {owner:?}");
        let _registered = table.map(validate_table);
        let fact = SystemFact::new(FactSource::Sidecar(owner.clone()), "result/sample".to_owned(), vec![0, 255, 17]);
        let mut records = Records::default();
        assert_eq!(records.push_system(fact).is_ok(), valid, "records {owner:?}");
    }
}
