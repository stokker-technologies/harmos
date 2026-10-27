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
use std::sync::OnceLock;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::{Direction, Entry, OperationDeclaration, OperationDefinition};
use crate::catalog::{Catalog, Member, Projection, Visitor};
use crate::config::SchemaSettings;
use crate::schema::declaration::Definition as SchemaDefinition;
use crate::schema::graph::{Kind, Schema, View};
use crate::schema::{self, Described};

/* === Constants === */

static INITIALIZE_CONTRACT: OnceLock<Contract> = OnceLock::new();
static SHUTDOWN_CONTRACT: OnceLock<Contract> = OnceLock::new();

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DecodedContract")]
pub struct Contract {
    operation: OperationDefinition,
    fingerprint: crate::Fingerprint,
    codec: Codec,
    schema: Schema,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    grpc: Option<super::Grpc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecodedContract {
    #[serde(with = "super::DecodedOperationDefinition")]
    operation: OperationDefinition,

    fingerprint: crate::Fingerprint,
    codec: Codec,
    schema: Schema,

    #[serde(default)]
    grpc: Option<super::Grpc>,
}

pub struct WireDeclaration {
    pub operation: Entry,
    pub codec: &'static str,
    pub response_codec: &'static str,
    pub input_codec: Option<&'static str>,
    pub output_codec: Option<&'static str>,
    pub request: SchemaDefinition,
    pub response: SchemaDefinition,
    pub input: Option<SchemaDefinition>,
    pub output: Option<SchemaDefinition>,
    pub grpc: Option<super::Grpc>,
}

pub struct Contracts;

// Wire Contracts Projected From Resource-Bound Operation Handlers.

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

    #[error("wire operation `{id}` has an invalid contract fingerprint")]
    Fingerprint { id: String },

    #[error("wire operation uses unsupported codec `{codec}`")]
    Codec { codec: String },

    #[error("invalid {direction} schema for wire operation `{id}`: {source}")]
    Schema {
        id: String,
        direction: &'static str,

        #[source]
        source: schema::Error,
    },
}

/* === Traits === */

/* === Implementations === */

impl Codec {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Postcard => "harmos.postcard.v1",
        }
    }
}

impl WireDeclaration {
    pub const fn of<O: OperationDeclaration>() -> Self {
        Self {
            operation: Entry::of::<O>(),
            codec: <O::Request as crate::Message>::CODEC,
            response_codec: <O::Response as crate::Message>::CODEC,
            input_codec: if O::Input::STREAMED { Some(<<O::Input as super::Direction>::Item as crate::Message>::CODEC) } else { None },
            output_codec: if O::Output::STREAMED { Some(<<O::Output as super::Direction>::Item as crate::Message>::CODEC) } else { None },
            request: <O::Request as Described>::DEFINITION,
            response: <O::Response as Described>::DEFINITION,
            input: direction::<O::Input>(),
            output: direction::<O::Output>(),
            grpc: O::GRPC,
        }
    }

    pub fn contract(&self) -> Result<Contract, ContractError> {
        let operation = self.operation.definition()?;
        for codec in [Some(self.codec), Some(self.response_codec), self.input_codec, self.output_codec].into_iter().flatten() {
            if codec != "harmos.postcard.v1" {
                return Err(ContractError::Codec { codec: codec.to_owned() });
            }
        }
        let mut roots = vec![("request", self.request), ("response", self.response)];
        if let Some(input) = self.input {
            roots.push(("input", input));
        }
        if let Some(output) = self.output {
            roots.push(("output", output));
        }
        let schema = Schema::from_declarations(&roots, &SchemaSettings::default()).map_err(|source| ContractError::Schema {
            id: operation.id.to_string(),
            direction: "contract",
            source: source.into(),
        })?;
        Contract::new(operation.clone(), operation.fingerprint, Codec::Postcard, schema)?.with_grpc(self.grpc.clone())
    }
}

impl Contract {
    pub fn new(
        operation: OperationDefinition, fingerprint: crate::Fingerprint, codec: Codec, schema: Schema,
    ) -> Result<Self, ContractError> {
        operation.validate()?;
        Self::canonical(operation, fingerprint, codec, schema)
    }

    fn canonical(
        operation: OperationDefinition, fingerprint: crate::Fingerprint, codec: Codec, schema: Schema,
    ) -> Result<Self, ContractError> {
        let invalid = |source: schema::graph::Error| ContractError::Schema {
            id: operation.id.to_string(),
            direction: "contract",
            source: source.into(),
        };
        schema.prepare(&SchemaSettings::default()).map_err(invalid)?;
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
            && fields.get(2).is_none_or(|field| field.name == "input" || field.name == "output")
            && fields.get(3).is_none_or(|field| fields[2].name == "input" && field.name == "output");
        if !valid {
            return Err(invalid(schema::graph::Error::Field {
                field: "contract directions".into(),
                reason: "expected request, response, and optional input/output roots",
            }));
        }
        let contract = Self { operation, fingerprint, codec, schema, grpc: None };
        let computed = contract.computed_fingerprint()?;
        if fingerprint != computed || contract.operation.fingerprint != computed {
            return Err(ContractError::Fingerprint { id: contract.operation.id.to_string() });
        }
        Ok(contract)
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
        action: crate::model::operations::lifecycle::Action, policy: super::OperationPolicy,
    ) -> Result<Self, ContractError> {
        use crate::model::operations::lifecycle::{self, Action};
        policy.validate().map_err(crate::Error::from)?;
        let cache = match action {
            Action::Initialize => &INITIALIZE_CONTRACT,
            Action::Shutdown => &SHUTDOWN_CONTRACT,
        };
        let template = if let Some(contract) = cache.get() {
            contract
        } else {
            let request = action.request();
            let request_schema = Schema::from_declaration(request, &SchemaSettings::default()).map_err(|source| ContractError::Schema {
                id: action.id().into(),
                direction: "request",
                source: source.into(),
            })?;
            let request_root = request_schema.root().map_err(|source| ContractError::Schema {
                id: action.id().into(),
                direction: "request",
                source: source.into(),
            })?;
            let request_fingerprint = crate::schema::identity::fingerprint(request_root, Codec::Postcard.id())
                .map_err(|source| ContractError::Schema { id: action.id().into(), direction: "request", source: source.into() })?;
            let fingerprint =
                crate::schema::identity::operation(action.id(), request_fingerprint, <() as crate::Portable>::fingerprint(), None, None);
            let operation = OperationDefinition {
                id: crate::Identifier::new(action.id()).map_err(crate::Error::from)?,
                description: String::new(),
                policy: lifecycle::POLICY,
                visibility: super::Visibility::Private,
                replay: super::ReplaySafety::Unknown,
                fingerprint,
            };
            let schema =
                Schema::from_declarations(&[("request", request), ("response", <() as Described>::DEFINITION)], &SchemaSettings::default())
                    .map_err(|source| ContractError::Schema { id: action.id().into(), direction: "contract", source: source.into() })?;
            let contract = Self::canonical(operation, fingerprint, Codec::Postcard, schema)?;
            cache.get_or_init(|| contract)
        };
        let mut contract = template.clone();
        contract.operation.policy = policy;
        Ok(contract)
    }

    // The Contract Of A Built-In Operation Under The Reserved `harmos.` Namespace,
    // Which Ordinary Declarations Cannot Claim.
    pub(crate) fn reserved<O: OperationDeclaration>(policy: super::OperationPolicy) -> Result<Self, ContractError> {
        policy.validate().map_err(crate::Error::from)?;
        let operation = OperationDefinition {
            id: crate::Identifier::new(O::ID).map_err(crate::Error::from)?,
            description: O::DESCRIPTION.to_owned(),
            policy,
            visibility: O::VISIBILITY,
            replay: O::REPLAY,
            fingerprint: (Entry::of::<O>().fingerprint)()?,
        };
        let schema = Schema::from_declarations(
            &[("request", <O::Request as Described>::DEFINITION), ("response", <O::Response as Described>::DEFINITION)],
            &SchemaSettings::default(),
        )
        .map_err(|source| ContractError::Schema { id: operation.id.to_string(), direction: "contract", source: source.into() })?;
        Self::canonical(operation.clone(), operation.fingerprint, Codec::Postcard, schema)
    }

    pub fn operation(&self) -> &OperationDefinition {
        &self.operation
    }

    pub fn fingerprint(&self) -> crate::Fingerprint {
        self.fingerprint
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
            && self.fingerprint == requirement.fingerprint
            && self.codec == requirement.codec
            && ["request", "response", "input", "output"].into_iter().all(|name| {
                match (self.direction(name), requirement.direction(name)) {
                    (None, None) => true,
                    (Some(left), Some(right)) => self
                        .schema
                        .equivalent(left.index(), &requirement.schema, right.index(), &SchemaSettings::default())
                        .unwrap_or(false),
                    _ => false,
                }
            })
    }

    fn computed_fingerprint(&self) -> Result<crate::Fingerprint, ContractError> {
        let payload = |view| {
            schema::identity::fingerprint(view, self.codec.id()).map_err(|source| ContractError::Schema {
                id: self.operation.id.to_string(),
                direction: "identity",
                source: source.into(),
            })
        };
        Ok(schema::identity::operation(
            self.operation.id.as_str(),
            payload(self.request())?,
            payload(self.response())?,
            self.input().map(payload).transpose()?,
            self.output().map(payload).transpose()?,
        ))
    }

    fn direction(&self, name: &str) -> Option<View<'_>> {
        let field = self.schema.definitions[self.schema.root].iter().find(|field| field.name == name)?;
        let Kind::Message { definition } = field.kind else {
            unreachable!("validated contract direction");
        };
        Some(self.schema.view(definition).expect("validated direction reference"))
    }
}

impl TryFrom<DecodedContract> for Contract {
    type Error = ContractError;

    fn try_from(decoded: DecodedContract) -> Result<Self, Self::Error> {
        if !crate::reserved(decoded.operation.id.as_str()) {
            return Self::new(decoded.operation, decoded.fingerprint, decoded.codec, decoded.schema)?.with_grpc(decoded.grpc);
        }

        // Read Only Exact SDK Metadata; Ordinary Declaration Admission Still Rejects These Names.
        use crate::model::operations::lifecycle::{Action, Descriptor};
        let refused = || crate::Error::ReservedOperation { id: decoded.operation.id.to_string() }.into();
        let expected = match decoded.operation.id.as_str() {
            crate::observers::Control::ID => crate::observers::contract()?,
            "harmos_initialize" => Descriptor::new(Action::Initialize, decoded.operation.policy).map_err(|_| refused())?.contract().clone(),
            "harmos_shutdown" => Descriptor::new(Action::Shutdown, decoded.operation.policy).map_err(|_| refused())?.contract().clone(),
            _ => return Err(refused()),
        };
        let identity_matches =
            decoded.operation == expected.operation && decoded.fingerprint == expected.fingerprint && decoded.codec == expected.codec;
        let schema_matches = decoded.schema == expected.schema && decoded.grpc == expected.grpc;
        if !identity_matches || !schema_matches {
            return Err(refused());
        }
        Ok(expected)
    }
}

impl Visitor for Contracts {
    type Entry = WireDeclaration;
}

impl<M: OperationDeclaration> Member<M> for Contracts {
    const ENTRY: WireDeclaration = WireDeclaration::of::<M>();
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

    pub fn from_tree(entries: &[crate::catalog::Node<WireDeclaration>]) -> Result<Vec<Contract>, ContractError> {
        let mut ids = BTreeSet::new();
        let mut contracts = Vec::new();
        crate::catalog::try_for_each(entries, |declaration| append(declaration, &mut ids, &mut contracts))?;
        Ok(contracts)
    }

    pub fn from_projection<P: Projection<WireDeclaration>>(entries: &P) -> Result<Vec<Contract>, ContractError> {
        let mut ids = BTreeSet::new();
        let mut contracts = Vec::with_capacity(P::LEN);
        entries.try_for_each(|declaration| append(declaration, &mut ids, &mut contracts))?;
        Ok(contracts)
    }
}

/* === Public Functions === */

/* === Private Functions === */

const fn direction<D: super::Direction>() -> Option<SchemaDefinition> {
    if D::STREAMED { Some(<D::Item as Described>::DEFINITION) } else { None }
}

fn append(
    declaration: &WireDeclaration, ids: &mut BTreeSet<crate::Identifier>, contracts: &mut Vec<Contract>,
) -> Result<(), ContractError> {
    let contract = declaration.contract()?;
    if !ids.insert(contract.operation.id.clone()) {
        return Err(crate::Error::DuplicateOperation { id: contract.operation.id.to_string() }.into());
    }
    contracts.push(contract);
    Ok(())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::operations::lifecycle::{self, Action, Descriptor};
    use crate::operations::{Backoff, Jitter, RetryPolicy};
    use serde_json::json;

    fn sdk_contracts() -> [Contract; 3] {
        let policy = super::super::OperationPolicy {
            timeout: std::time::Duration::from_secs(17),
            attempt_timeout: std::time::Duration::from_secs(11),
            ..lifecycle::POLICY
        };
        [
            crate::observers::contract().unwrap(),
            Descriptor::new(Action::Initialize, policy).unwrap().contract().clone(),
            Descriptor::new(Action::Shutdown, policy).unwrap().contract().clone(),
        ]
    }

    #[test]
    fn sdk_metadata_roundtrip() {
        for original in sdk_contracts() {
            let encoded = serde_json::to_vec(&original).unwrap();
            let decoded: Contract = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(decoded, original);
        }
    }

    #[test]
    fn sdk_metadata_forgery() {
        let delay = std::time::Duration::from_secs(1);
        let backoff = Backoff::Fixed { delay, jitter: Jitter::None };
        for original in sdk_contracts() {
            let encoded = serde_json::to_value(&original).unwrap();
            for (pointer, replacement) in [
                ("/operation/id", json!("harmos.forged")),
                ("/operation/description", json!("forged SDK contract")),
                ("/operation/visibility", json!("shared")),
                ("/operation/policy/attempt_timeout/secs", json!(0)),
                ("/operation/policy/retries", json!(RetryPolicy::OnTransient { max_retries: 1, backoff })),
                ("/fingerprint", json!("00".repeat(32))),
                ("/schema", json!({"root": 0, "definitions": [[]]})),
            ] {
                let mut forged = encoded.clone();
                *forged.pointer_mut(pointer).unwrap() = replacement;
                assert!(serde_json::from_value::<Contract>(forged).is_err(), "{pointer}");
            }
            let mut exposed = encoded;
            exposed["grpc"] = json!({"package": "forged.v1", "service": "Exposed", "method": "Invoke"});
            assert!(serde_json::from_value::<Contract>(exposed).is_err());
        }
    }

    #[test]
    fn sdk_lifecycle_refuses_invalid() {
        let delay = std::time::Duration::from_secs(1);
        let backoff = Backoff::Fixed { delay, jitter: Jitter::None };
        let descriptors = [
            Descriptor::new(Action::Initialize, lifecycle::POLICY).unwrap(),
            Descriptor::new(Action::Shutdown, lifecycle::POLICY).unwrap(),
        ];
        for descriptor in &descriptors {
            assert_eq!(Contract::from(descriptor.clone()), *descriptor.contract());
            let encoded = serde_json::to_value(descriptor).unwrap();
            assert_eq!(serde_json::from_value::<Descriptor>(encoded.clone()).unwrap(), *descriptor);
            for (pointer, replacement) in [
                ("/fingerprint", json!("00".repeat(32))),
                ("/policy/attempt_timeout/secs", json!(0)),
                ("/policy/retries", json!(RetryPolicy::OnTransient { max_retries: 1, backoff })),
            ] {
                let mut forged = encoded.clone();
                *forged.pointer_mut(pointer).unwrap() = replacement;
                assert!(serde_json::from_value::<Descriptor>(forged).is_err(), "{pointer}");
            }
        }
        let mut manifest = crate::inventory::contracts::Manifest {
            facility: "guest".into(),
            owner: "fixture".into(),
            provides: Vec::new(),
            requires: Vec::new(),
            lifecycle: descriptors,
        };
        crate::inventory::contracts::validate(&manifest).unwrap();
        manifest.lifecycle.swap(0, 1);
        assert!(crate::inventory::contracts::validate(&manifest).is_err());
    }

    #[test]
    fn reserved_operations_refused() {
        use crate::inventory::contracts::{Manifest, validate};
        for contract in sdk_contracts() {
            assert!(contract.operation().validate().is_err());
            let definition = serde_json::to_value(contract.operation()).unwrap();
            assert!(serde_json::from_value::<OperationDefinition>(definition).is_err());

            let mut manifest = Manifest {
                facility: "sidecar".into(),
                owner: "fixture".into(),
                provides: Vec::new(),
                requires: Vec::new(),
                lifecycle: [
                    Descriptor::new(Action::Initialize, lifecycle::POLICY).unwrap(),
                    Descriptor::new(Action::Shutdown, lifecycle::POLICY).unwrap(),
                ],
            };
            validate(&manifest).unwrap();
            manifest.provides.push(contract);
            assert!(validate(&manifest).is_err());
            let encoded = serde_json::to_value(&manifest).unwrap();
            assert!(serde_json::from_value::<Manifest>(encoded).is_err());
        }
    }
}
