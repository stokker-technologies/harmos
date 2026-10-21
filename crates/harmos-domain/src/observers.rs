/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The built-in observe control `harmos.observe`. A sidecar that declares
// observers serves it: the host submits it with the observer's identity, the
// sidecar initializes and ticks that observer for as long as the invocation
// runs, and cancelling the invocation stops and cleans it up through the
// ordinary operation lifecycle.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
// Third Party Imports

// Local Imports
use crate::operations::{Contract, ContractError, OperationPolicy, ReplaySafety, RestartPolicy, RetryPolicy, UNBOUNDED, Visibility};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};

/* === Constants === */

// The observe control runs until the host cancels it.
pub const POLICY: OperationPolicy =
    OperationPolicy { timeout: UNBOUNDED, attempt_timeout: UNBOUNDED, retries: RetryPolicy::Never, restart: RestartPolicy::Never };

/* === Macros === */

crate::declare!(Control {
    id: "harmos.observe",
    version: 1,
    policy: POLICY,
    response: (),
    visibility: Visibility::Host,
    replay: ReplaySafety::Idempotent,
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Control {
    pub observer: String,
    pub generation: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

crate::message::postcard_product!(Control { observer: String, generation: u64 });

impl crate::MemorySize for Control {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.observer.capacity())
    }
}

impl Described for Control {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Control));
    const FIELDS: &'static [Field] = &[
        Field { name: Cow::Borrowed("observer"), tag: 1, kind: Kind::String, cardinality: Cardinality::Required },
        Field { name: Cow::Borrowed("generation"), tag: 2, kind: Kind::U64, cardinality: Cardinality::Required },
    ];
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

/* === Public Functions === */

// The Contract Every Observer-Declaring Sidecar Serves.
pub fn contract() -> Result<Contract, ContractError> {
    Contract::reserved::<Control>(POLICY)
}

/* === Private Functions === */

/* === Tests === */
