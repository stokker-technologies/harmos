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
use super::ProtobufExt;
use super::{Direction, Entry, OperationDefinition};
use crate::catalog::Visitor;
use harmos_domain::config::SchemaSettings;
use harmos_domain::schema::declaration::Definition as SchemaDefinition;
use harmos_domain::schema::graph::{Kind, Schema, View};
use harmos_domain::schema::{self, Described};

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
    fingerprint: harmos_domain::Fingerprint,
    codec: Codec,
    schema: Schema,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    grpc: Option<super::Grpc>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    protobuf: Option<super::Protobuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecodedContract {
    #[serde(with = "super::DecodedOperationDefinition")]
    operation: OperationDefinition,

    fingerprint: harmos_domain::Fingerprint,
    codec: Codec,
    schema: Schema,

    #[serde(default)]
    grpc: Option<super::Grpc>,

    #[serde(default)]
    protobuf: Option<super::Protobuf>,
}

#[derive(Clone)]
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

// Wire Contracts Projected From Resource-Bound Operation Handlers.
pub struct Contracts;

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    Protobuf,
}

#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    #[error("invalid or reserved gRPC binding name")]
    Grpc,

    #[error("invalid protobuf operation: {0}")]
    Protobuf(String),

    #[error("Invoke `{id}`: only Public operations can declare a client-facing gRPC method")]
    RestrictedGrpc { id: String },

    #[error("invalid wire operation metadata: {0}")]
    Operation(#[from] crate::operations::Error),

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
            Self::Protobuf => "harmos.protobuf.v1",
        }
    }
}

impl WireDeclaration {
    pub const fn visibility(mut self, visibility: super::Visibility) -> Self {
        self.operation.visibility = visibility;
        self
    }

    pub const fn description(mut self, description: &'static str) -> Self {
        self.operation.description = description;
        self
    }

    pub const fn replay(mut self, replay: super::ReplaySafety) -> Self {
        self.operation.replay = replay;
        self
    }

    pub const fn grpc(mut self, grpc: super::Grpc) -> Self {
        assert!(self.grpc.is_none(), "gRPC binding is declared only once");
        let previous = self.grpc.replace(grpc);
        std::mem::forget(previous);
        self
    }

    pub const fn request<Q: super::RequestShape>(id: &'static str, policy: super::OperationPolicy) -> Self {
        Self {
            operation: Entry::request::<Q>(id, policy),
            codec: <Q::Request as harmos_domain::Message>::CODEC,
            response_codec: <Q::Response as harmos_domain::Message>::CODEC,
            input_codec: if Q::Input::STREAMED { Some(<<Q::Input as Direction>::Item as harmos_domain::Message>::CODEC) } else { None },
            output_codec: if Q::Output::STREAMED { Some(<<Q::Output as Direction>::Item as harmos_domain::Message>::CODEC) } else { None },
            request: <Q::Request as Described>::DEFINITION,
            response: <Q::Response as Described>::DEFINITION,
            input: direction::<Q::Input>(),
            output: direction::<Q::Output>(),
            grpc: None,
        }
    }

    pub const fn copied(&self) -> Self {
        let grpc = match &self.grpc {
            None => None,
            Some(grpc) => {
                let (std::borrow::Cow::Borrowed(package), std::borrow::Cow::Borrowed(service), std::borrow::Cow::Borrowed(method)) =
                    (&grpc.package, &grpc.service, &grpc.method)
                else {
                    panic!("static gRPC declarations must borrow names");
                };
                let mut copied = super::Grpc::new(package, service, method);
                copied.managed = grpc.managed;
                Some(copied)
            }
        };
        Self {
            operation: self.operation,
            codec: self.codec,
            response_codec: self.response_codec,
            input_codec: self.input_codec,
            output_codec: self.output_codec,
            request: self.request,
            response: self.response,
            input: self.input,
            output: self.output,
            grpc,
        }
    }

    pub fn contract(&self) -> Result<Contract, ContractError> {
        self.contract_with_settings(&SchemaSettings::default())
    }

    pub fn contract_with_settings(&self, settings: &SchemaSettings) -> Result<Contract, ContractError> {
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
        let schema = Schema::from_declarations(&roots, settings).map_err(|source| ContractError::Schema {
            id: operation.id.to_string(),
            direction: "contract",
            source: source.into(),
        })?;
        Contract::new_with_settings(operation.clone(), operation.fingerprint, Codec::Protobuf, schema, settings)?
            .with_grpc(self.grpc.clone())
    }
}

impl Contract {
    pub fn new(
        operation: OperationDefinition, fingerprint: harmos_domain::Fingerprint, codec: Codec, schema: Schema,
    ) -> Result<Self, ContractError> {
        Self::new_with_settings(operation, fingerprint, codec, schema, &SchemaSettings::default())
    }

    pub fn new_with_settings(
        operation: OperationDefinition, fingerprint: harmos_domain::Fingerprint, codec: Codec, schema: Schema, settings: &SchemaSettings,
    ) -> Result<Self, ContractError> {
        operation.validate()?;
        Self::canonical(operation, Some(fingerprint), codec, schema, settings)
    }

    // Runtime Authors Supply The Schema; Harmos Derives Its Canonical Identity.
    pub fn from_schema(
        operation: OperationDefinition, codec: Codec, schema: Schema, settings: &SchemaSettings,
    ) -> Result<Self, ContractError> {
        operation.validate()?;
        Self::canonical(operation, None, codec, schema, settings)
    }

    fn canonical(
        operation: OperationDefinition, fingerprint: Option<harmos_domain::Fingerprint>, codec: Codec, schema: Schema,
        settings: &SchemaSettings,
    ) -> Result<Self, ContractError> {
        let invalid = |source: schema::graph::Error| ContractError::Schema {
            id: operation.id.to_string(),
            direction: "contract",
            source: source.into(),
        };
        schema.prepare(settings).map_err(invalid)?;
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
        let mut contract = Self {
            fingerprint: fingerprint.unwrap_or(operation.fingerprint),
            operation,
            codec,
            schema,
            grpc: None,
            protobuf: None,
        };
        let computed = contract.computed_fingerprint()?;
        if fingerprint.is_some_and(|fingerprint| fingerprint != computed || contract.operation.fingerprint != computed) {
            return Err(ContractError::Fingerprint { id: contract.operation.id.to_string() });
        }
        contract.fingerprint = computed;
        contract.operation.fingerprint = computed;
        Ok(contract)
    }

    pub fn from_protobuf(
        operation: OperationDefinition, protobuf: super::Protobuf, binding: Option<super::Grpc>, settings: &SchemaSettings,
    ) -> Result<Self, ContractError> {
        harmos_protocol::schema::operation_identity(operation.id.as_str()).map_err(|error| ContractError::Protobuf(error.to_string()))?;
        if operation.id.as_str().starts_with("harmos.plugin.host.v1.") {
            use prost::Message;
            use prost_reflect::ReflectMessage;
            let root = harmos_protocol::plugin::host::TimeGetRequest::default().descriptor();
            let pool = root.parent_pool();
            let (service, method) = operation.id.as_str().split_once('/').ok_or(ContractError::Grpc)?;
            let service = pool.get_service_by_name(service).ok_or(ContractError::Grpc)?;
            let method = service.methods().find(|candidate| candidate.name() == method).ok_or(ContractError::Grpc)?;
            let request = method.input().full_name().to_owned();
            let response = method.output().full_name().to_owned();
            let types = prost_types::FileDescriptorSet {
                file: pool.files().map(|file| file.file_descriptor_proto().clone()).collect(),
            };
            let types = harmos_protocol::schema::operation_types(&types, &[&request, &response])
                .map_err(|error| ContractError::Protobuf(error.to_string()))?;
            let expected = super::Protobuf {
                descriptors: types.encode_to_vec(),
                request,
                response: response.clone(),
                input: None,
                output: method.is_server_streaming().then_some(response),
            };
            if protobuf != expected || binding.is_some() || operation.visibility != super::Visibility::Public {
                return Err(ContractError::Protobuf("context declaration differs from its authoritative protocol".into()));
            }
            operation.policy.validate().map_err(crate::operations::Error::from)?;
            return Self::generated(operation, protobuf, None, settings);
        }
        if crate::reserved(operation.id.as_str()) {
            use prost::Message;
            use prost_reflect::ReflectMessage;
            let root = harmos_protocol::client::OperationsListRequest::default().descriptor();
            let pool = root.parent_pool();
            let (service, method) = operation.id.as_str().split_once('/').ok_or(ContractError::Grpc)?;
            if !service.starts_with("harmos.client.v1.") {
                return Err(crate::operations::Error::ReservedOperation { id: operation.id.to_string() }.into());
            }
            let service = pool.get_service_by_name(service).ok_or(ContractError::Grpc)?;
            let method = service.methods().find(|candidate| candidate.name() == method).ok_or(ContractError::Grpc)?;
            let request = method.input().full_name().to_owned();
            let response = method.output().full_name().to_owned();
            let types = prost_types::FileDescriptorSet {
                file: root.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
            };
            let expected = harmos_protocol::schema::operation_types(&types, &[&request, &response])
                .map_err(|error| ContractError::Protobuf(error.to_string()))?;
            let expected = super::Protobuf {
                descriptors: expected.encode_to_vec(),
                request,
                response: response.clone(),
                input: None,
                output: method.is_server_streaming().then_some(response),
            };
            let mapping = Some(super::Grpc {
                package: service.parent_file().package_name().to_owned().into(),
                service: service.name().to_owned().into(),
                method: method.name().to_owned().into(),
                managed: false,
            });
            if protobuf != expected || binding != mapping || operation.visibility != super::Visibility::Public {
                return Err(ContractError::Protobuf("built-in declaration differs from its authoritative protocol".into()));
            }
            operation.policy.validate().map_err(crate::operations::Error::from)?;
        } else {
            operation.policy.validate().map_err(crate::operations::Error::from)?;
        }
        Self::generated(operation, protobuf, binding, settings)
    }

    /// Construct Only the Protocol-Owned Host Clock Contracts.
    pub fn clock(sleep: bool, policy: super::OperationPolicy, settings: &SchemaSettings) -> Result<Self, ContractError> {
        use prost::Message;
        policy.validate().map_err(crate::operations::Error::from)?;
        let (id, request, response, method) = if sleep {
            ("harmos.client.v1.Time/Sleep", "harmos.client.v1.TimeSleepRequest", "harmos.client.v1.TimeSleepResponse", "Sleep")
        } else {
            ("harmos.client.v1.Time/Get", "harmos.client.v1.TimeGetRequest", "harmos.client.v1.TimeGetResponse", "Get")
        };
        let operation = OperationDefinition {
            id: harmos_domain::Identifier::new(id).map_err(crate::operations::Error::from)?,
            description: String::new(),
            visibility: super::Visibility::Public,
            policy,
            replay: super::ReplaySafety::Unknown,
            fingerprint: harmos_domain::Fingerprint([0; 32]),
        };
        let protobuf = super::Protobuf {
            descriptors: harmos_protocol::clock::client_types().encode_to_vec(),
            request: request.into(),
            response: response.into(),
            input: None,
            output: None,
        };
        Self::generated(operation, protobuf, Some(super::Grpc::new("harmos.client.v1", "Time", method)), settings)
    }

    pub(crate) fn generated(
        mut operation: OperationDefinition, protobuf: super::Protobuf, binding: Option<super::Grpc>, settings: &SchemaSettings,
    ) -> Result<Self, ContractError> {
        let schema = protobuf.schema()?;
        schema.prepare(settings).map_err(|source| ContractError::Schema {
            id: operation.id.to_string(),
            direction: "protobuf",
            source: source.into(),
        })?;
        let fingerprint = protobuf.fingerprint(operation.id.as_str())?;
        operation.fingerprint = fingerprint;
        Self {
            operation,
            fingerprint,
            codec: Codec::Protobuf,
            schema,
            grpc: None,
            protobuf: Some(protobuf),
        }
        .with_grpc(binding)
    }

    pub fn protobuf(&self) -> Option<&super::Protobuf> {
        self.protobuf.as_ref()
    }

    pub fn grpc(&self) -> Option<&super::Grpc> {
        self.grpc.as_ref()
    }

    pub fn with_grpc(mut self, binding: Option<super::Grpc>) -> Result<Self, ContractError> {
        if let Some(binding) = &binding {
            if self.operation.visibility != super::Visibility::Public {
                return Err(ContractError::RestrictedGrpc { id: self.operation.id.to_string() });
            }
            binding.validate()?;
            if binding.managed && self.operation.policy.cancellation_timeout.is_zero() {
                return Err(ContractError::Protobuf("Declare positive cancellation grace for a managed final result".into()));
            }
        }
        self.grpc = binding;
        Ok(self)
    }

    // Only the Closed SDK Action Can Construct Reserved Metadata
    pub(crate) fn lifecycle(
        action: crate::model::operations::lifecycle::Action, policy: super::OperationPolicy,
    ) -> Result<Self, ContractError> {
        use crate::model::operations::lifecycle::{self, Action};
        policy.validate().map_err(crate::operations::Error::from)?;
        let cache = match action {
            Action::Initialize => &INITIALIZE_CONTRACT,
            Action::Shutdown => &SHUTDOWN_CONTRACT,
        };
        let template = if let Some(contract) = cache.get() {
            contract
        } else {
            use prost::Message;
            let request = match action {
                Action::Initialize => "harmos.v1.GuestInitialize",
                Action::Shutdown => "harmos.v1.GuestShutdown",
            };
            let mut types = harmos_protocol::guest::types().clone();
            types.file.push(prost_types::FileDescriptorProto {
                name: Some("google/protobuf/empty.proto".into()),
                package: Some("google.protobuf".into()),
                syntax: Some("proto3".into()),
                message_type: vec![prost_types::DescriptorProto {
                    name: Some("Empty".into()),
                    ..Default::default()
                }],
                ..Default::default()
            });
            let operation = OperationDefinition {
                id: harmos_domain::Identifier::new(action.id()).map_err(crate::operations::Error::from)?,
                description: String::new(),
                policy: lifecycle::POLICY,
                visibility: super::Visibility::Protected,
                replay: super::ReplaySafety::Unknown,
                fingerprint: harmos_domain::Fingerprint([0; 32]),
            };
            let protobuf = super::Protobuf {
                descriptors: types.encode_to_vec(),
                request: request.into(),
                response: "google.protobuf.Empty".into(),
                input: None,
                output: None,
            };
            let contract = Self::generated(operation, protobuf, None, &SchemaSettings::default())?;
            cache.get_or_init(|| contract)
        };
        let mut contract = template.clone();
        contract.operation.policy = policy;
        Ok(contract)
    }

    // The Contract Of A Built-In Operation Under The Reserved `harmos.` Namespace,
    // Which Ordinary Declarations Cannot Claim.
    pub(crate) fn reserved<O: super::RequestShape>(specification: &super::OperationSpec<O>) -> Result<Self, ContractError> {
        let operation = specification.declared()?;
        operation.policy.validate().map_err(crate::operations::Error::from)?;
        let schema = Schema::from_declarations(
            &[("request", <O::Request as Described>::DEFINITION), ("response", <O::Response as Described>::DEFINITION)],
            &SchemaSettings::default(),
        )
        .map_err(|source| ContractError::Schema {
            id: operation.id.to_string(),
            direction: "contract",
            source: source.into(),
        })?;
        Self::canonical(operation.clone(), Some(operation.fingerprint), Codec::Protobuf, schema, &SchemaSettings::default())
    }

    /// Declare the Corresponding Context Capability With Its Own Wrappers.
    pub fn context(&self, settings: &SchemaSettings) -> Result<Option<Self>, ContractError> {
        use prost::Message;
        use prost_reflect::ReflectMessage;
        let Some(binding) = self.grpc() else {
            return Ok(None);
        };
        if binding.package != "harmos.client.v1" {
            return Ok(None);
        }
        let root = harmos_protocol::plugin::host::TimeGetRequest::default().descriptor();
        let pool = root.parent_pool();
        let service_name = format!("harmos.plugin.host.v1.{}", binding.service);
        let Some(service) = pool.get_service_by_name(&service_name) else {
            return Ok(None);
        };
        let Some(method) = service.methods().find(|method| method.name() == binding.method) else {
            return Ok(None);
        };
        let request = method.input().full_name().to_owned();
        let response = method.output().full_name().to_owned();
        let types = prost_types::FileDescriptorSet {
            file: pool.files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        let types = harmos_protocol::schema::operation_types(&types, &[&request, &response])
            .map_err(|error| ContractError::Protobuf(error.to_string()))?;
        let mut operation = self.operation.clone();
        operation.id =
            harmos_domain::Identifier::new(format!("{service_name}/{}", method.name())).map_err(crate::operations::Error::from)?;
        let protobuf = super::Protobuf {
            descriptors: types.encode_to_vec(),
            request,
            response: response.clone(),
            input: None,
            output: method.is_server_streaming().then_some(response),
        };
        Self::generated(operation, protobuf, None, settings).map(Some)
    }

    pub fn operation(&self) -> &OperationDefinition {
        &self.operation
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Result<Self, ContractError> {
        self.operation.id = harmos_domain::Identifier::new(id.into()).map_err(crate::operations::Error::from)?;
        if self.protobuf.is_some() {
            if crate::reserved(self.operation.id.as_str()) {
                return Err(crate::operations::Error::ReservedOperation { id: self.operation.id.to_string() }.into());
            }
            harmos_protocol::schema::operation_identity(self.operation.id.as_str())
                .map_err(|error| ContractError::Protobuf(error.to_string()))?;
            self.operation.policy.validate().map_err(crate::operations::Error::from)?;
        } else {
            self.operation.validate()?;
        }
        self.fingerprint = self.computed_fingerprint()?;
        self.operation.fingerprint = self.fingerprint;
        Ok(self)
    }

    pub fn fingerprint(&self) -> harmos_domain::Fingerprint {
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
        self.compatible_with_settings(requirement, &SchemaSettings::default())
    }

    pub fn compatible_with_settings(&self, requirement: &Self, settings: &SchemaSettings) -> bool {
        self.operation.id == requirement.operation.id
            && self.fingerprint == requirement.fingerprint
            && self.codec == requirement.codec
            && ["request", "response", "input", "output"].into_iter().all(|name| {
                match (self.direction(name), requirement.direction(name)) {
                    (None, None) => true,
                    (Some(left), Some(right)) => {
                        self.schema.equivalent(left.index(), &requirement.schema, right.index(), settings).unwrap_or(false)
                    }
                    _ => false,
                }
            })
    }

    fn computed_fingerprint(&self) -> Result<harmos_domain::Fingerprint, ContractError> {
        if let Some(protobuf) = &self.protobuf {
            return protobuf.fingerprint(self.operation.id.as_str());
        }
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
        if decoded.operation.id.as_str() == crate::observers::CONTROL_OPERATION.entry().id
            || matches!(decoded.operation.id.as_str(), "harmos.initialize" | "harmos.shutdown")
        {
            // Read Only Exact SDK Metadata; Ordinary Declaration Admission Still Rejects These Names.
            use crate::model::operations::lifecycle::{Action, Descriptor};
            let refused = || crate::operations::Error::ReservedOperation { id: decoded.operation.id.to_string() }.into();
            let expected = match decoded.operation.id.as_str() {
                id if id == crate::observers::CONTROL_OPERATION.entry().id => crate::observers::contract()?,
                "harmos.initialize" => {
                    Descriptor::new(Action::Initialize, decoded.operation.policy).map_err(|_| refused())?.contract().clone()
                }
                "harmos.shutdown" => Descriptor::new(Action::Shutdown, decoded.operation.policy).map_err(|_| refused())?.contract().clone(),
                _ => return Err(refused()),
            };
            let identity_matches =
                decoded.operation == expected.operation && decoded.fingerprint == expected.fingerprint && decoded.codec == expected.codec;
            let schema_matches =
                decoded.schema == expected.schema && decoded.grpc == expected.grpc && decoded.protobuf == expected.protobuf;
            if !identity_matches || !schema_matches {
                return Err(refused());
            }
            return Ok(expected);
        }
        if let Some(protobuf) = decoded.protobuf {
            let expected = if matches!(decoded.operation.id.as_str(), "harmos.client.v1.Time/Sleep" | "harmos.client.v1.Time/Get") {
                let clock = Self::clock(
                    decoded.operation.id.as_str() == "harmos.client.v1.Time/Sleep",
                    decoded.operation.policy,
                    &SchemaSettings::default(),
                )?;
                if clock.protobuf() != Some(&protobuf) || clock.grpc != decoded.grpc {
                    return Err(ContractError::Fingerprint { id: decoded.operation.id.to_string() });
                }
                clock
            } else {
                Self::from_protobuf(decoded.operation.clone(), protobuf, decoded.grpc, &SchemaSettings::default())?
            };
            if expected.fingerprint != decoded.fingerprint
                || expected.operation != decoded.operation
                || expected.schema != decoded.schema
                || decoded.codec != Codec::Protobuf
            {
                return Err(ContractError::Fingerprint { id: decoded.operation.id.to_string() });
            }
            return Ok(expected);
        }
        if !crate::reserved(decoded.operation.id.as_str()) {
            return Self::new(decoded.operation, decoded.fingerprint, decoded.codec, decoded.schema)?.with_grpc(decoded.grpc);
        }

        Err(crate::operations::Error::ReservedOperation { id: decoded.operation.id.to_string() }.into())
    }
}

impl Visitor for Contracts {
    type Entry = WireDeclaration;
}

impl Contracts {
    pub fn from_tree(entries: &[crate::catalog::Node<WireDeclaration>]) -> Result<Vec<Contract>, ContractError> {
        let mut ids = BTreeSet::new();
        let mut contracts = Vec::new();
        crate::catalog::try_for_each(entries, |declaration| append(declaration, &mut ids, &mut contracts))?;
        Ok(contracts)
    }
}

/* === Public Functions === */

/* === Private Functions === */

const fn direction<D: super::Direction>() -> Option<SchemaDefinition> {
    if D::STREAMED { Some(<D::Item as Described>::DEFINITION) } else { None }
}

fn append(
    declaration: &WireDeclaration, ids: &mut BTreeSet<harmos_domain::Identifier>, contracts: &mut Vec<Contract>,
) -> Result<(), ContractError> {
    let contract = declaration.contract()?;
    if !ids.insert(contract.operation.id.clone()) {
        return Err(crate::operations::Error::DuplicateOperation { id: contract.operation.id.to_string() }.into());
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

    #[test]
    fn application_wire_contracts_keep_native_identity_and_fingerprint_checks() {
        let template = Contract::clock(false, lifecycle::POLICY, &SchemaSettings::default()).unwrap();
        for id in [
            "example.step.0",
            "example.plugin.v1.Lifecycle/Initialize",
            "example.context.v1.Prompts/Create",
            "example.v1.jobs.Progress/Get",
        ] {
            let mut operation = template.operation().clone();
            operation.id = harmos_domain::Identifier::new(id).unwrap();
            operation.visibility = super::super::Visibility::Protected;
            assert!(operation.validate().is_err(), "native identity unexpectedly accepted {id}");
            let contract =
                Contract::from_protobuf(operation, template.protobuf().unwrap().clone(), None, &SchemaSettings::default()).unwrap();
            let encoded = serde_json::to_value(&contract).unwrap();
            assert_eq!(serde_json::from_value::<Contract>(encoded.clone()).unwrap(), contract);
            let renamed = contract.clone().with_id("example.criterion.0").unwrap();
            assert_eq!(renamed.operation().id.as_str(), "example.criterion.0");
            assert_eq!(renamed.fingerprint(), template.protobuf().unwrap().fingerprint("example.criterion.0").unwrap());
            assert!(contract.with_id("harmos.plugin.v1.Instance/Invoke").is_err());
            let mut forged = encoded;
            forged["fingerprint"] = json!("00".repeat(32));
            assert!(serde_json::from_value::<Contract>(forged).is_err(), "forged fingerprint accepted for {id}");
        }
        for id in ["example..step.0", "example.plugin.v1.Steps/", "example.plugin.v1.Steps/Execute/Again"] {
            let mut operation = template.operation().clone();
            operation.id = harmos_domain::Identifier::new(id).unwrap();
            let error =
                Contract::from_protobuf(operation, template.protobuf().unwrap().clone(), None, &SchemaSettings::default()).unwrap_err();
            assert!(error.to_string().contains(id));
            assert!(error.to_string().contains("Declare"));
        }
    }

    fn sdk_contracts() -> [Contract; 3] {
        let policy = super::super::OperationPolicy {
            timeout: std::time::Duration::from_secs(17),
            attempt_timeout: std::time::Duration::from_secs(11),
            cancellation_timeout: std::time::Duration::ZERO,
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
                ("/operation/visibility", json!("public")),
                ("/operation/policy/attempt_timeout/secs", json!(0)),
                ("/operation/policy/retries", json!(RetryPolicy::OnTransient { total_attempts: 2, backoff })),
                ("/fingerprint", json!("00".repeat(32))),
                ("/schema", json!({"root": 0, "definitions": [[]]})),
            ] {
                let mut forged = encoded.clone();
                *forged.pointer_mut(pointer).unwrap() = replacement;
                assert!(serde_json::from_value::<Contract>(forged).is_err(), "{pointer}");
            }
            if let Some(protobuf) = original.protobuf() {
                let mut forged = encoded.clone();
                let mut protobuf = protobuf.clone();
                protobuf.descriptors.clear();
                forged["protobuf"] = serde_json::to_value(protobuf).unwrap();
                assert!(serde_json::from_value::<Contract>(forged).is_err());
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
                ("/policy/retries", json!(RetryPolicy::OnTransient { total_attempts: 2, backoff })),
            ] {
                let mut forged = encoded.clone();
                *forged.pointer_mut(pointer).unwrap() = replacement;
                assert!(serde_json::from_value::<Descriptor>(forged).is_err(), "{pointer}");
            }
        }
    }

    #[test]
    fn reserved_operations_refused() {
        for contract in sdk_contracts() {
            assert!(contract.operation().validate().is_err());
            let definition = serde_json::to_value(contract.operation()).unwrap();
            assert!(serde_json::from_value::<OperationDefinition>(definition).is_err());
        }
    }
}
