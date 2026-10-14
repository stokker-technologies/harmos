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
    impl<P> Leaf for super::Initialize<P> {}
    impl<P> Leaf for super::Shutdown<P> {}
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::borrow::Cow;
use std::marker::PhantomData;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::{
    Contract, ContractError, OperationDeclaration, OperationPolicy, PolicyError, RestartPolicy,
    RetryPolicy, WireOperation,
};
use crate::catalog::{self, Member};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "grpc", derive(prost::Message))]
#[cfg_attr(not(feature = "grpc"), derive(Debug, Default))]
pub struct InitializeRequest {
    #[cfg_attr(feature = "grpc", prost(bytes = "vec", tag = "1"))]
    pub config_json: Vec<u8>,
    #[cfg_attr(feature = "grpc", prost(bytes = "vec", tag = "2"))]
    pub generation_context: Vec<u8>,
}

pub struct Initialize<P>(PhantomData<fn() -> P>);

pub struct Shutdown<P>(PhantomData<fn() -> P>);

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

pub enum Catalog<P> {
    Initialize(Initialize<P>),
    Shutdown(Shutdown<P>),
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
crate::message::postcard_product!(InitializeRequest { config_json: Vec<u8>, generation_context: Vec<u8> });
impl<P: Policy> Catalog<P> {
    pub fn bind<I, S, C>(
        table: &mut super::WireTable<C>,
        resources: std::sync::Arc<I::Resources>,
    ) -> Result<(), super::ServingError>
    where
        C: Send + Sync + 'static,
        I: for<'a> super::Operation<
                Declaration = Initialize<P>,
                Context<'a> = &'a C,
                Endpoints = (),
            >,
        S: for<'a> super::Operation<
                Declaration = Shutdown<P>,
                Resources = I::Resources,
                Context<'a> = &'a C,
                Endpoints = (),
            >,
    {
        Descriptor::of_initialize::<P>()?;
        Descriptor::of_shutdown::<P>()?;
        table.bind_owned_lifecycle::<I>(std::sync::Arc::clone(&resources))?;
        table.bind_owned_lifecycle::<S>(resources)
    }
}

impl<P: Policy> SdkOperation for Initialize<P> {
    const ACTION: Action = Action::Initialize;
}

impl<P: Policy> SdkOperation for Shutdown<P> {
    const ACTION: Action = Action::Shutdown;
}

impl Described for InitializeRequest {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(InitializeRequest));
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("config_json"),
            tag: 1,
            kind: Kind::Bytes,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("generation_context"),
            tag: 2,
            kind: Kind::Bytes,
            cardinality: Cardinality::Required,
        },
    ];
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl<P: Policy> OperationDeclaration for Initialize<P> {
    type Request = InitializeRequest;
    type Response = ();
    const ID: &'static str = "harmos.initialize";
    const POLICY: OperationPolicy = P::INITIALIZATION_POLICY;
}

impl<P: Policy> OperationDeclaration for Shutdown<P> {
    type Request = ();
    type Response = ();
    const ID: &'static str = "harmos.shutdown";
    const POLICY: OperationPolicy = P::SHUTDOWN_POLICY;
}

impl<P: Policy> WireOperation for Initialize<P> {
    const VERSION: u32 = 1;
}

impl<P: Policy> WireOperation for Shutdown<P> {
    const VERSION: u32 = 1;
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
        Self {
            action: descriptor.action,
            version: descriptor.contract.version(),
            policy: descriptor.contract.operation().policy,
        }
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

impl<P, V> catalog::Catalog<V> for Catalog<P>
where
    V: Member<Initialize<P>> + Member<Shutdown<P>>,
{
    type Entries = [V::Entry; 2];
    const ENTRIES: Self::Entries = [
        <V as Member<Initialize<P>>>::ENTRY,
        <V as Member<Shutdown<P>>>::ENTRY,
    ];
}

impl<P> catalog::Contains<Initialize<P>> for Catalog<P> {}

impl<P> catalog::Contains<Shutdown<P>> for Catalog<P> {}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
