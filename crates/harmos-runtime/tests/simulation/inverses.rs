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

use harmos_runtime::simulation::SimulatedRuntime;
use harmos_runtime::{
    Application, DefinitionId, Identified, Runtime, runtime::Catalog, runtime::Registration, state::PrincipalId, state::Transaction,
};
use harmos_runtime::{state::DecodeError, state::Kind};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Model {
    gain: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
struct SetGain(i64);

impl Transaction<Model> for SetGain {
    type Error = std::convert::Infallible;

    fn check(&self, _model: &Model) -> Result<(), Self::Error> {
        Ok(())
    }

    fn apply(&self, model: &mut Model) {
        model.gain = self.0;
    }
}

impl harmos_runtime::state::TransactionCatalog<Model> for SetGain {}
impl Identified for SetGain {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("sim.gain"), 1)
    }
}

struct Amplifier;

#[harmos_runtime::lanes]
pub enum AmplifierLanes {
    #[default]
    Open,
}

impl Application for Amplifier {
    type Lanes = AmplifierLanes;
    type State = Model;
    type Transaction = SetGain;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

#[derive(Clone)]
struct Faithful;

#[derive(Clone)]
struct Wrong;

fn decode_nothing(_id: &DefinitionId, _version: u32, _bytes: &[u8]) -> Result<Option<SetGain>, DecodeError> {
    Ok(None)
}

impl Registration<Amplifier> for Faithful {
    fn register(self) -> Catalog<Amplifier> {
        Catalog::Transactions {
            definitions: declared(),
            decode: decode_nothing,
            inverses: Some(|_change, before| Some(SetGain(before.gain))),
        }
    }
}

impl Registration<Amplifier> for Wrong {
    fn register(self) -> Catalog<Amplifier> {
        Catalog::Transactions {
            definitions: declared(),
            decode: decode_nothing,
            inverses: Some(|change, _before| Some(SetGain(change.0))),
        }
    }
}

fn declared() -> Vec<(Kind, DefinitionId, u32)> {
    vec![(Kind::Transaction, DefinitionId::new("sim.gain"), 1)]
}

fn scripted<C: Registration<Amplifier> + 'static>(catalog: C) -> SimulatedRuntime<Amplifier> {
    let mut sim = Runtime::<Amplifier>::builder(Model::default(), ())
        .register(catalog)
        .entropy_seed(1)
        .simulate()
        .expect("a memory-only simulation boots");

    for gain in [3, 9] {
        sim.commit(PrincipalId::new("tester"), SetGain(gain)).expect("accepted");
    }

    sim
}

// SIM-AC008: a declared inverse restores the state its change replaced.

#[test]
fn inverse_restores_change() {
    scripted(Faithful).assert_inverses_restore();
}

#[test]
fn inverse_fails_law() {
    let sim = scripted(Wrong);
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sim.assert_inverses_restore();
    }))
    .expect_err("an inverse that restores nothing is not an inverse");
    let message = failure.downcast_ref::<String>().expect("divergence diagnostic");
    assert_eq!(
        message,
        "the inverse of the change at Position(1) did not restore the state it changed\n  before:   Model { gain: 0 }\n  restored: Model { gain: 3 }",
        "the first change reports its exact position, before and restored values"
    );
}
