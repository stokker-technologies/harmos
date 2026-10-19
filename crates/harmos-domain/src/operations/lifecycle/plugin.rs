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
use crate::operations::{Contract, ContractError, OperationDeclaration, OperationPolicy, PolicyError, RestartPolicy, RetryPolicy};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};

/* === Constants === */

// The Declared Bound Of Both Actions; Each Plugin Binds Its Own `Policy`.
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(30),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

crate::declare!(Initialize { id: "harmos.initialize", version: 1, policy: POLICY, response: () });

crate::declare!(Shutdown { id: "harmos.shutdown", version: 1, policy: POLICY, response: () });

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Initialize {
    pub config_json: Vec<u8>,
    pub generation_context: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Shutdown {}

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
    version: u32,
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

    #[error("SDK lifecycle operations are request-scoped and never restart")]
    Resident,

    #[error("unsupported SDK lifecycle semantic version {0}")]
    Version(u32),

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

crate::message::postcard_product!(Initialize { config_json: Vec<u8>, generation_context: Vec<u8> });
crate::message::postcard_product!(Shutdown {});
impl SdkOperation for Initialize {
    const ACTION: Action = Action::Initialize;
}

impl SdkOperation for Shutdown {
    const ACTION: Action = Action::Shutdown;
}

impl Described for Initialize {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Initialize));
    const FIELDS: &'static [Field] = &[
        Field { name: Cow::Borrowed("config_json"), tag: 1, kind: Kind::Bytes, cardinality: Cardinality::Required },
        Field { name: Cow::Borrowed("generation_context"), tag: 2, kind: Kind::Bytes, cardinality: Cardinality::Required },
    ];
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl Described for Shutdown {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Shutdown));
    const FIELDS: &'static [Field] = &[];
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl MemorySize for Initialize {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.config_json.capacity()).saturating_add(self.generation_context.capacity())
    }
}

impl MemorySize for Shutdown {
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
        if policy.restart != RestartPolicy::Never {
            return Err(Error::Resident);
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

impl From<Descriptor> for EncodedDescriptor {
    fn from(descriptor: Descriptor) -> Self {
        Self { action: descriptor.action, version: descriptor.contract.version(), policy: descriptor.contract.operation().policy }
    }
}

impl TryFrom<EncodedDescriptor> for Descriptor {
    type Error = Error;

    fn try_from(encoded: EncodedDescriptor) -> Result<Self, Error> {
        if encoded.version != 1 {
            return Err(Error::Version(encoded.version));
        }
        Self::new(encoded.action, encoded.policy)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
