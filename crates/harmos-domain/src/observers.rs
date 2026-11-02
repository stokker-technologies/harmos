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
use crate::operations::{Contract, ContractError, OperationPolicy, ReplaySafety, RetryPolicy, UNBOUNDED, Visibility};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};

/* === Constants === */

pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: UNBOUNDED,
    attempt_timeout: UNBOUNDED,
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

pub struct Control;

impl crate::operations::OperationDeclaration for Control {
    type Request = ControlRequest;
    type Response = ();
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "harmos_observe";
    const POLICY: crate::operations::OperationPolicy = POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Host;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ControlRequest {
    pub observer: String,
    pub generation: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

crate::message::postcard_product!(ControlRequest { observer: String, generation: u64 });

impl crate::MemorySize for ControlRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.observer.capacity())
    }
}

impl Described for ControlRequest {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(ControlRequest));
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("observer"),
            tag: 1,
            kind: Kind::String,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("generation"),
            tag: 2,
            kind: Kind::U64,
            cardinality: Cardinality::Required,
        },
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
