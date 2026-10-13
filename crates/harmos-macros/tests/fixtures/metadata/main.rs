/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod operations;
mod streams;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */
const RECORD_DEMAND: harmos::DemandKey = harmos::DemandKey::new("fixture", "record");

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Native;

#[derive(Default)]
struct Watch;

#[derive(Default)]
struct Unselected;

struct Catalog;

/* === Enums === */
#[harmos::catalog]
enum Observers {
    Watch(Watch),
}

/* === Traits === */
trait ProductPlugin {}

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for Native {
    const INITIALIZATION_POLICY: harmos::operations::OperationPolicy = operations::SDK_POLICY;
    const SHUTDOWN_POLICY: harmos::operations::OperationPolicy = operations::SDK_POLICY;
}

#[harmos::observer(
    id = "selected.watch",
    interval_ms = 17,
    streams(self::streams::Reading),
    demand_keys(RECORD_DEMAND)
)]
impl Watch {
    async fn observe(
        &mut self,
        _: &harmos::ObserverContext<'_, ()>,
    ) -> Result<(), harmos::Refusal> {
        panic!("inspection never invokes observer code")
    }
}

#[harmos::observer(
    id = "unselected.watch",
    interval_ms = 19,
    streams(self::streams::Ungranted)
)]
impl Unselected {
    async fn observe(
        &mut self,
        _: &harmos::ObserverContext<'_, ()>,
    ) -> Result<(), harmos::Refusal> {
        panic!("unselected observer is inert")
    }
}

impl harmos::plugin::Contribution for Catalog {
    const NAMESPACE: &'static str = "fixture.catalog";
    const SECTION: harmos::inventory::Section =
        harmos::inventory::Section::new(Self::NAMESPACE, r#"{"kinds":["probe"]}"#);
    fn register(registrar: &mut harmos::Registrar) -> Result<(), harmos::RegistrationError> {
        registrar.register(Self::NAMESPACE, 1, &["probe"])
    }
}

#[harmos::plugin(description = "Quoted \"probe\" line \\ \u{3bc}\u{1}", id = "cargo-native", contributions(Catalog), context = harmos::Supervisor, config = String,
    provides = self::operations::NativeOperations, requires = (), observers = Observers,
    observer_controls(operations::Start, operations::Stop), streams(self::streams::Reading), restart = "never")]
impl ProductPlugin for Native {}

/* === Public Functions === */

/* === Private Functions === */
fn main() {
    // Exercise The Same Inert Marker Projection The SDK Owner Retains
    if std::env::args().nth(1).as_deref() == Some("--markers") {
        use std::io::Write;
        let mut output = std::io::stdout().lock();
        let markers =
            std::hint::black_box(<Native as harmos::plugin::Declared>::markers().unwrap());
        for marker in markers {
            output
                .write_all(marker)
                .expect("markers reach standard output");
        }
        output.flush().expect("markers reach standard output");
    }
}

/* === Tests === */
