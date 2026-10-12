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
use std::collections::BTreeSet;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::{Definition, Entry, OperationDeclaration};
use crate::catalog::{Catalog, Member, Projection, Visitor};
use crate::schema::{self, Described, Field};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DecodedContract")]
pub struct Contract {
    operation: Definition,
    version: u32,
    codec: Codec,
    request: Vec<Field>,
    response: Vec<Field>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    input: Option<Vec<Field>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output: Option<Vec<Field>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grpc: Option<super::Grpc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecodedContract {
    operation: Definition,
    version: u32,
    codec: Codec,
    request: Vec<Field>,
    response: Vec<Field>,
    #[serde(default)]
    input: Option<Vec<Field>>,
    #[serde(default)]
    output: Option<Vec<Field>>,
    #[serde(default)]
    grpc: Option<super::Grpc>,
}

pub struct WireDeclaration {
    pub operation: Entry,
    pub version: u32,
    pub request: &'static [Field],
    pub response: &'static [Field],
    pub input: Option<&'static [Field]>,
    pub output: Option<&'static [Field]>,
    pub grpc: Option<super::Grpc>,
}

pub struct Contracts;

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    Postcard,
}

#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    #[error("invalid or reserved gRPC binding name")]
    Grpc,
    #[error("invalid wire operation metadata: {0}")]
    Operation(#[from] crate::Error),
    #[error("wire operation `{id}` has a zero semantic version")]
    Version { id: String },
    #[error("invalid {direction} schema for wire operation `{id}`: {source}")]
    Schema {
        id: String,
        direction: &'static str,
        #[source]
        source: schema::Error,
    },
}

/* === Traits === */
pub trait WireOperation: OperationDeclaration {
    const VERSION: u32;
    const GRPC: Option<super::Grpc> = None;

    #[cfg(feature = "grpc")]
    fn visit_grpc<V: super::GrpcVisitor>(_visitor: V) -> Option<V::Entry> {
        None
    }

    const INPUT: Option<&'static [Field]> = None;
    const OUTPUT: Option<&'static [Field]> = None;
}

pub trait WireVisitor: Sized {
    type Entry;
    fn wire<O>(self) -> Self::Entry
    where
        O: WireOperation,
        O::Request: crate::Message + Described,
        O::Response: crate::Message + Described;
    fn unary<O>(self) -> Self::Entry
    where
        O: WireOperation,
        O::Request: crate::Message + Described,
        O::Response: crate::Message + Described,
    {
        self.wire::<O>()
    }

    fn upload<O>(self) -> Self::Entry
    where
        O: WireOperation + super::InputDeclaration,
        O::Request: crate::Message + Described,
        O::Response: crate::Message + Described,
        O::Item: crate::Message + crate::MemorySize,
    {
        self.wire::<O>()
    }

    fn download<O>(self) -> Self::Entry
    where
        O: WireOperation + super::OutputDeclaration,
        O::Request: crate::Message + Described,
        O::Response: crate::Message + Described,
        O::Item: crate::Message + crate::MemorySize,
    {
        self.wire::<O>()
    }

    fn exchange<O>(self) -> Self::Entry
    where
        O: WireOperation + super::InputDeclaration + super::OutputDeclaration,
        O::Request: crate::Message + Described,
        O::Response: crate::Message + Described,
        <O as super::InputDeclaration>::Item: crate::Message + crate::MemorySize,
        <O as super::OutputDeclaration>::Item: crate::Message + crate::MemorySize,
    {
        self.wire::<O>()
    }
}

/* === Implementations === */
impl WireDeclaration {
    pub const fn of<O>() -> Self
    where
        O: WireOperation,
        O::Request: Described,
        O::Response: Described,
    {
        Self {
            operation: Entry::of::<O>(),
            version: O::VERSION,
            request: O::Request::FIELDS,
            response: O::Response::FIELDS,
            input: O::INPUT,
            output: O::OUTPUT,
            grpc: O::GRPC,
        }
    }

    pub fn contract(&self) -> Result<Contract, ContractError> {
        let operation = self.operation.definition()?;
        if self.version == 0 {
            return Err(ContractError::Version {
                id: operation.id.to_string(),
            });
        }
        // Preflight Borrowed Schemas Before Cloning Their Owned Representation
        for (direction, fields) in [
            ("request", Some(self.request)),
            ("response", Some(self.response)),
            ("input", self.input),
            ("output", self.output),
        ] {
            let Some(fields) = fields else { continue };
            schema::validate(fields).map_err(|source| ContractError::Schema {
                id: operation.id.to_string(),
                direction,
                source,
            })?;
        }
        Contract::new(
            operation,
            self.version,
            Codec::Postcard,
            self.request.to_vec(),
            self.response.to_vec(),
        )?
        .with_streams(
            self.input.map(<[Field]>::to_vec),
            self.output.map(<[Field]>::to_vec),
        )?
        .with_grpc(self.grpc.clone())
    }
}

impl Contract {
    pub fn new(
        operation: Definition,
        version: u32,
        codec: Codec,
        request: Vec<Field>,
        response: Vec<Field>,
    ) -> Result<Self, ContractError> {
        operation.validate()?;
        Self::canonical(operation, version, codec, request, response)
    }

    fn canonical(
        operation: Definition,
        version: u32,
        codec: Codec,
        request: Vec<Field>,
        response: Vec<Field>,
    ) -> Result<Self, ContractError> {
        if version == 0 {
            return Err(ContractError::Version {
                id: operation.id.to_string(),
            });
        }
        for (direction, fields) in [
            ("request", request.as_slice()),
            ("response", response.as_slice()),
        ] {
            schema::validate(fields).map_err(|source| ContractError::Schema {
                id: operation.id.to_string(),
                direction,
                source,
            })?;
        }

        Ok(Self {
            operation,
            version,
            codec,
            request,
            response,
            input: None,
            output: None,
            grpc: None,
        })
    }

    pub fn grpc(&self) -> Option<&super::Grpc> {
        self.grpc.as_ref()
    }

    pub fn with_grpc(mut self, binding: Option<super::Grpc>) -> Result<Self, ContractError> {
        if let Some(binding) = &binding {
            binding.validate()?;
        }
        self.grpc = binding;
        Ok(self)
    }

    pub fn with_streams(
        mut self,
        mut input: Option<Vec<Field>>,
        mut output: Option<Vec<Field>>,
    ) -> Result<Self, ContractError> {
        for (direction, fields) in [("input", &mut input), ("output", &mut output)] {
            if let Some(fields) = fields {
                schema::validate(fields).map_err(|source| ContractError::Schema {
                    id: self.operation.id.to_string(),
                    direction,
                    source,
                })?;
            }
        }
        self.input = input;
        self.output = output;
        Ok(self)
    }

    // Only The Closed SDK Action Can Construct Reserved Metadata
    pub(super) fn lifecycle(
        action: super::lifecycle::Action,
        policy: super::OperationPolicy,
    ) -> Result<Self, ContractError> {
        use super::lifecycle::{Action, InitializeRequest};
        let (id, request) = match action {
            Action::Initialize => ("harmos.initialize", InitializeRequest::FIELDS.to_vec()),
            Action::Shutdown => ("harmos.shutdown", Vec::new()),
        };
        policy.validate().map_err(crate::Error::from)?;
        let operation = Definition {
            id: crate::Identifier::new(id).map_err(crate::Error::from)?,
            description: String::new(),
            policy,
            visibility: super::Visibility::Private,
            replay: super::ReplaySafety::Unknown,
        };
        Self::canonical(operation, 1, Codec::Postcard, request, Vec::new())
    }

    pub fn operation(&self) -> &Definition {
        &self.operation
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn codec(&self) -> Codec {
        self.codec
    }

    pub fn request(&self) -> &[Field] {
        &self.request
    }

    pub fn response(&self) -> &[Field] {
        &self.response
    }

    pub fn input(&self) -> Option<&[Field]> {
        self.input.as_deref()
    }

    pub fn output(&self) -> Option<&[Field]> {
        self.output.as_deref()
    }

    pub fn compatible(&self, requirement: &Self) -> bool {
        self.operation.id == requirement.operation.id
            && self.version == requirement.version
            && self.codec == requirement.codec
            && self.request == requirement.request
            && self.response == requirement.response
            && self.input == requirement.input
            && self.output == requirement.output
    }
}

impl TryFrom<DecodedContract> for Contract {
    type Error = ContractError;

    fn try_from(decoded: DecodedContract) -> Result<Self, Self::Error> {
        Self::new(
            decoded.operation,
            decoded.version,
            decoded.codec,
            decoded.request,
            decoded.response,
        )?
        .with_streams(decoded.input, decoded.output)?
        .with_grpc(decoded.grpc)
    }
}

impl Visitor for Contracts {
    type Entry = WireDeclaration;
}

impl<M> Member<M> for Contracts
where
    M: super::Declares,
    M::Declaration: WireOperation,
    <M::Declaration as OperationDeclaration>::Request: Described,
    <M::Declaration as OperationDeclaration>::Response: Described,
{
    const ENTRY: WireDeclaration = WireDeclaration::of::<M::Declaration>();
}

impl Contracts {
    pub fn of<C: Catalog<Self>>() -> Result<Vec<Contract>, ContractError> {
        Self::from_projection(&C::ENTRIES)
    }

    pub fn from_entries(entries: &[WireDeclaration]) -> Result<Vec<Contract>, ContractError> {
        let mut ids = BTreeSet::new();
        let mut contracts = Vec::with_capacity(entries.len());
        for declaration in entries {
            append(declaration, &mut ids, &mut contracts)?;
        }
        Ok(contracts)
    }

    pub fn from_tree(
        entries: &[crate::catalog::Node<WireDeclaration>],
    ) -> Result<Vec<Contract>, ContractError> {
        let mut ids = BTreeSet::new();
        let mut contracts = Vec::new();
        crate::catalog::try_for_each(entries, |declaration| {
            append(declaration, &mut ids, &mut contracts)
        })?;
        Ok(contracts)
    }

    pub fn from_projection<P: Projection<WireDeclaration>>(
        entries: &P,
    ) -> Result<Vec<Contract>, ContractError> {
        let mut ids = BTreeSet::new();
        let mut contracts = Vec::with_capacity(P::LEN);
        entries.try_for_each(|declaration| append(declaration, &mut ids, &mut contracts))?;
        Ok(contracts)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn append(
    declaration: &WireDeclaration,
    ids: &mut BTreeSet<crate::Identifier>,
    contracts: &mut Vec<Contract>,
) -> Result<(), ContractError> {
    let contract = declaration.contract()?;
    if !ids.insert(contract.operation.id.clone()) {
        return Err(crate::Error::DuplicateOperation {
            id: contract.operation.id.to_string(),
        }
        .into());
    }
    contracts.push(contract);
    Ok(())
}

/* === Tests === */
