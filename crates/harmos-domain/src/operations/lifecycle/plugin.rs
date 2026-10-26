/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod sealed {
    pub trait Leaf {}
    impl Leaf for super::Initialize {}
    impl Leaf for super::Shutdown {}
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::MemorySize;
use crate::operations::{Contract, ContractError, OperationDeclaration, OperationPolicy, PolicyError, RetryPolicy};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};

/* === Constants === */

// The Declared Bound Of Both Actions; Each Plugin Binds Its Own `Policy`.
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(30),
    retries: RetryPolicy::Never,
};

/* === Macros === */

pub struct Initialize;

impl crate::operations::OperationDeclaration for Initialize {
    type Request = InitializeRequest;
    type Response = ();
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "harmos_initialize";
    const POLICY: crate::operations::OperationPolicy = POLICY;
}

impl crate::operations::RequestFor for InitializeRequest {
    type Operation = Initialize;
}

pub struct Shutdown;

impl crate::operations::OperationDeclaration for Shutdown {
    type Request = ShutdownRequest;
    type Response = ();
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "harmos_shutdown";
    const POLICY: crate::operations::OperationPolicy = POLICY;
}

impl crate::operations::RequestFor for ShutdownRequest {
    type Operation = Shutdown;
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, PartialEq, Debug, Default)]
pub struct InitializeRequest {
    pub config_json: Vec<u8>,
    pub generation_context: Vec<u8>,
    pub assignments: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ShutdownRequest {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "EncodedDescriptor", try_from = "EncodedDescriptor")]
pub struct Descriptor {
    action: Action,
    contract: Contract,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedDescriptor {
    action: Action,
    fingerprint: crate::Fingerprint,
    policy: OperationPolicy,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Initialize,
    Shutdown,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid SDK lifecycle policy: {0}")]
    Policy(#[from] PolicyError),

    #[error("SDK lifecycle operations require explicit retry, never automatic replay")]
    AutomaticRetry,

    #[error("unsupported SDK lifecycle fingerprint {0}")]
    Fingerprint(crate::Fingerprint),

    #[error("invalid SDK lifecycle contract: {0}")]
    Contract(#[from] ContractError),
}

/* === Traits === */

pub trait Policy: 'static {
    const INITIALIZATION_POLICY: OperationPolicy;
    const SHUTDOWN_POLICY: OperationPolicy;
}

pub trait SdkOperation: OperationDeclaration + sealed::Leaf {
    const ACTION: Action;
}

/* === Implementations === */

crate::message::postcard_product!(InitializeRequest { config_json: Vec<u8>, generation_context: Vec<u8>, assignments: Vec<u8> });
crate::message::postcard_product!(ShutdownRequest {});
impl SdkOperation for Initialize {
    const ACTION: Action = Action::Initialize;
}

impl SdkOperation for Shutdown {
    const ACTION: Action = Action::Shutdown;
}

impl Described for InitializeRequest {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(InitializeRequest));
    const FIELDS: &'static [Field] = &[
        Field { name: Cow::Borrowed("config_json"), tag: 1, kind: Kind::Bytes, cardinality: Cardinality::Required },
        Field { name: Cow::Borrowed("generation_context"), tag: 2, kind: Kind::Bytes, cardinality: Cardinality::Required },
        Field { name: Cow::Borrowed("assignments"), tag: 3, kind: Kind::Bytes, cardinality: Cardinality::Required },
    ];
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl Described for ShutdownRequest {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(ShutdownRequest));
    const FIELDS: &'static [Field] = &[];
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl MemorySize for InitializeRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.config_json.capacity())
            .saturating_add(self.generation_context.capacity())
            .saturating_add(self.assignments.capacity())
    }
}

impl MemorySize for ShutdownRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl Descriptor {
    pub fn new(action: Action, policy: OperationPolicy) -> Result<Self, Error> {
        policy.validate()?;
        if policy.retries != RetryPolicy::Never {
            return Err(Error::AutomaticRetry);
        }
        let contract = Contract::lifecycle(action, policy)?;
        Ok(Self { action, contract })
    }

    pub fn of_initialize<P: Policy>() -> Result<Self, Error> {
        Self::new(Action::Initialize, P::INITIALIZATION_POLICY)
    }

    pub fn of_shutdown<P: Policy>() -> Result<Self, Error> {
        Self::new(Action::Shutdown, P::SHUTDOWN_POLICY)
    }

    pub fn action(&self) -> Action {
        self.action
    }

    pub fn contract(&self) -> &Contract {
        &self.contract
    }
}

impl From<Descriptor> for Contract {
    fn from(descriptor: Descriptor) -> Self {
        descriptor.contract
    }
}

impl From<Descriptor> for EncodedDescriptor {
    fn from(descriptor: Descriptor) -> Self {
        Self { action: descriptor.action, fingerprint: descriptor.contract.fingerprint(), policy: descriptor.contract.operation().policy }
    }
}

impl TryFrom<EncodedDescriptor> for Descriptor {
    type Error = Error;

    fn try_from(encoded: EncodedDescriptor) -> Result<Self, Error> {
        let descriptor = Self::new(encoded.action, encoded.policy)?;
        if descriptor.contract.fingerprint() != encoded.fingerprint {
            return Err(Error::Fingerprint(encoded.fingerprint));
        }
        Ok(descriptor)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
