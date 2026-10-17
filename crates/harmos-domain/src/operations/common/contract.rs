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
use crate::config::SchemaSettings;
use crate::schema::declaration::Definition as SchemaDefinition;
use crate::schema::graph::{Kind, Schema, View};
use crate::schema::{self, Described};

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
    schema: Schema,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grpc: Option<super::Grpc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecodedContract {
    operation: Definition,
    version: u32,
    codec: Codec,
    schema: Schema,
    #[serde(default)]
    grpc: Option<super::Grpc>,
}

pub struct WireDeclaration {
    pub operation: Entry,
    pub version: u32,
    pub request: SchemaDefinition,
    pub response: SchemaDefinition,
    pub input: Option<SchemaDefinition>,
    pub output: Option<SchemaDefinition>,
    pub grpc: Option<super::Grpc>,
}

pub struct Contracts;

// Wire Contracts Projected From Resource-Bound Operation Handlers.
pub struct AuthoredContracts;

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

    const INPUT: Option<SchemaDefinition> = None;
    const OUTPUT: Option<SchemaDefinition> = None;
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
            request: O::Request::DEFINITION,
            response: O::Response::DEFINITION,
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
        let mut roots = vec![("request", self.request), ("response", self.response)];
        if let Some(input) = self.input {
            roots.push(("input", input));
        }
        if let Some(output) = self.output {
            roots.push(("output", output));
        }
        let schema =
            Schema::from_declarations(&roots, &SchemaSettings::default()).map_err(|source| {
                ContractError::Schema {
                    id: operation.id.to_string(),
                    direction: "contract",
                    source: source.into(),
                }
            })?;
        Contract::new(operation, self.version, Codec::Postcard, schema)?
            .with_grpc(self.grpc.clone())
    }
}

impl Contract {
    pub fn new(
        operation: Definition,
        version: u32,
        codec: Codec,
        schema: Schema,
    ) -> Result<Self, ContractError> {
        operation.validate()?;
        Self::canonical(operation, version, codec, schema)
    }

    fn canonical(
        operation: Definition,
        version: u32,
        codec: Codec,
        schema: Schema,
    ) -> Result<Self, ContractError> {
        if version == 0 {
            return Err(ContractError::Version {
                id: operation.id.to_string(),
            });
        }
        let invalid = |source: schema::graph::Error| ContractError::Schema {
            id: operation.id.to_string(),
            direction: "contract",
            source: source.into(),
        };
        schema
            .prepare(&SchemaSettings::default())
            .map_err(invalid)?;
        let directions = schema.root().map_err(invalid)?;
        let fields = directions.fields();
        let valid = (2..=4).contains(&fields.len())
            && fields[0].name == "request"
            && fields[1].name == "response"
            && fields.iter().enumerate().all(|(index, field)| {
                field.tag == (index + 1) as u32
                    && field.cardinality == schema::Cardinality::Required
                    && matches!(field.kind, Kind::Message { .. })
            })
            && fields
                .get(2)
                .is_none_or(|field| field.name == "input" || field.name == "output")
            && fields
                .get(3)
                .is_none_or(|field| fields[2].name == "input" && field.name == "output");
        if !valid {
            return Err(invalid(schema::graph::Error::Field {
                field: "contract directions".into(),
                reason: "expected request, response, and optional input/output roots",
            }));
        }
        Ok(Self {
            operation,
            version,
            codec,
            schema,
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

    // Only The Closed SDK Action Can Construct Reserved Metadata
    pub(crate) fn lifecycle(
        action: crate::operations::lifecycle::Action,
        policy: super::OperationPolicy,
    ) -> Result<Self, ContractError> {
        use crate::operations::lifecycle::{Action, InitializeRequest};
        let (id, request) = match action {
            Action::Initialize => ("harmos.initialize", InitializeRequest::DEFINITION),
            Action::Shutdown => ("harmos.shutdown", <()>::DEFINITION),
        };
        policy.validate().map_err(crate::Error::from)?;
        let operation = Definition {
            id: crate::Identifier::new(id).map_err(crate::Error::from)?,
            description: String::new(),
            policy,
            visibility: super::Visibility::Private,
            replay: super::ReplaySafety::Unknown,
        };
        let schema = Schema::from_declarations(
            &[("request", request), ("response", <()>::DEFINITION)],
            &SchemaSettings::default(),
        )
        .map_err(|source| ContractError::Schema {
            id: operation.id.to_string(),
            direction: "contract",
            source: source.into(),
        })?;
        Self::canonical(operation, 1, Codec::Postcard, schema)
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

    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    pub fn request(&self) -> View<'_> {
        self.direction("request").expect("validated request root")
    }

    pub fn response(&self) -> View<'_> {
        self.direction("response").expect("validated response root")
    }

    pub fn input(&self) -> Option<View<'_>> {
        self.direction("input")
    }

    pub fn output(&self) -> Option<View<'_>> {
        self.direction("output")
    }

    pub fn compatible(&self, requirement: &Self) -> bool {
        self.operation.id == requirement.operation.id
            && self.version == requirement.version
            && self.codec == requirement.codec
            && ["request", "response", "input", "output"]
                .into_iter()
                .all(
                    |name| match (self.direction(name), requirement.direction(name)) {
                        (None, None) => true,
                        (Some(left), Some(right)) => self
                            .schema
                            .equivalent(
                                left.index(),
                                &requirement.schema,
                                right.index(),
                                &SchemaSettings::default(),
                            )
                            .unwrap_or(false),
                        _ => false,
                    },
                )
    }

    fn direction(&self, name: &str) -> Option<View<'_>> {
        let field = self.schema.definitions[self.schema.root]
            .iter()
            .find(|field| field.name == name)?;
        let Kind::Message { definition } = field.kind else {
            unreachable!("validated contract direction");
        };
        Some(
            self.schema
                .view(definition)
                .expect("validated direction reference"),
        )
    }
}

impl TryFrom<DecodedContract> for Contract {
    type Error = ContractError;

    fn try_from(decoded: DecodedContract) -> Result<Self, Self::Error> {
        Self::new(
            decoded.operation,
            decoded.version,
            decoded.codec,
            decoded.schema,
        )?
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

impl Visitor for AuthoredContracts {
    type Entry = WireDeclaration;
}

impl<T> Member<T> for AuthoredContracts
where
    T: super::Operation,
    T::Declaration: WireOperation,
    <T::Declaration as OperationDeclaration>::Request: Described,
    <T::Declaration as OperationDeclaration>::Response: Described,
{
    const ENTRY: WireDeclaration = WireDeclaration::of::<T::Declaration>();
}

impl AuthoredContracts {
    pub fn of<C: Catalog<Self>>() -> Result<Vec<Contract>, ContractError> {
        Contracts::from_projection(&C::ENTRIES)
    }
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
