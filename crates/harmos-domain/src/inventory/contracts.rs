/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use crate::model::operations::lifecycle;

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::PREFIX;
use crate::catalog::Node;
use crate::config::SchemaSettings;
use crate::operations::{Backoff, Contract, Entry, Jitter, OperationPolicy, ReplaySafety, RetryPolicy, Visibility, WireDeclaration};
use crate::plugin::{Grant, Requirement};
use crate::schema::declaration::Named;
use crate::schema::graph::Schema;

const ENTRY_LIMIT: usize = 4096;
const GRANT_LIMIT: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Decoded")]
pub struct Manifest {
    pub facility: String,
    pub owner: String,
    pub provides: Vec<Contract>,
    pub requires: Vec<Required>,
    pub lifecycle: [lifecycle::Descriptor; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Required {
    pub contract: Contract,

    #[serde(default)]
    pub grants: Vec<Grant>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub facility: String,
    pub id: String,
    pub version: String,
    pub description: String,
    pub config: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decoded {
    #[serde(default)]
    kind: Option<String>,

    facility: String,
    owner: String,
    provides: Vec<Contract>,
    requires: Vec<Required>,
    lifecycle: [lifecycle::Descriptor; 2],
}

// Inert Artifact Metadata Names Direction Roots Without Repeating Their Fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedContract {
    pub operation: NamedOperation,
    pub identity: String,
    pub payload_codec: String,
    pub response_codec: String,
    pub input_codec: Option<String>,
    pub output_codec: Option<String>,
    pub codec: crate::operations::Codec,
    pub request: String,
    pub response: String,

    #[serde(default)]
    pub input: Option<String>,

    #[serde(default)]
    pub output: Option<String>,

    #[serde(default)]
    pub grpc: Option<crate::operations::Grpc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedRequired {
    pub contract: NamedContract,

    #[serde(default)]
    pub grants: Vec<Grant>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedManifest {
    pub facility: String,
    pub owner: String,
    pub provides: Vec<NamedContract>,
    pub requires: Vec<NamedRequired>,
    pub lifecycle: [NamedLifecycle; 2],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedOperation {
    pub id: crate::Identifier,
    pub description: String,
    pub policy: OperationPolicy,
    pub visibility: Visibility,
    pub replay: ReplaySafety,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedLifecycle {
    pub action: lifecycle::Action,
    pub identity: String,
    pub policy: OperationPolicy,
}

pub(crate) struct Encoder<const N: usize> {
    pub(crate) bytes: [u8; N],
    pub(crate) cursor: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing complete {facility} contract inventory for `{owner}`")]
    Missing { facility: String, owner: String },

    #[error("multiple {facility} contract inventories for `{owner}`")]
    Duplicate { facility: String, owner: String },

    #[error("contract inventory exceeds its entry or fragment budget")]
    Budget,

    #[error("contract inventory requires a guest or sidecar facility and a valid artifact owner")]
    Identity,

    #[error("contract inventory has an invalid fragment kind")]
    Kind,

    #[error("requirement `{id}` carries a duplicate or invalid grant")]
    Grant { id: String },

    #[error("duplicate ordinary operation `{id}` in {direction}")]
    OperationDuplicate { direction: &'static str, id: String },

    #[error("invalid ordinary operation metadata: {0}")]
    Operation(#[from] crate::Error),

    #[error("SDK lifecycle inventory must contain initialization then shutdown")]
    LifecycleOrder,

    #[error("invalid SDK lifecycle inventory: {0}")]
    Lifecycle(#[from] lifecycle::Error),

    #[error("invalid shared contract schema: {0}")]
    Schema(#[from] crate::schema::graph::Error),

    #[error("invalid resolved contract: {0}")]
    Contract(#[from] crate::operations::ContractError),

    #[error("invalid contract inventory JSON: {0}")]
    Decode(#[from] serde_json::Error),
}

impl NamedContract {
    pub fn resolve(self, declarations: &[Named], settings: &mut SchemaSettings) -> Result<Contract, Error> {
        let mut roots = vec![("request", self.request.as_str()), ("response", self.response.as_str())];
        if let Some(input) = &self.input {
            roots.push(("input", input.as_str()));
        }
        if let Some(output) = &self.output {
            roots.push(("output", output.as_str()));
        }
        let schema = Schema::from_named_roots(&roots, declarations, settings)?;
        let schema = super::retain_schema(schema, settings)?;
        if self.identity != "harmos.operation.v1" {
            return Err(Error::Identity);
        }
        for codec in [Some(self.payload_codec), Some(self.response_codec), self.input_codec, self.output_codec].into_iter().flatten() {
            if codec != "harmos.postcard.v1" {
                return Err(crate::operations::ContractError::Codec { codec }.into());
            }
        }
        let fingerprint = crate::schema::identity::contract(self.operation.id.as_str(), &schema, self.codec.id())?;
        let operation = crate::OperationDefinition {
            id: self.operation.id,
            description: self.operation.description,
            policy: self.operation.policy,
            visibility: self.operation.visibility,
            replay: self.operation.replay,
            fingerprint,
        };
        Ok(Contract::new(operation, fingerprint, self.codec, schema)?.with_grpc(self.grpc)?)
    }
}

impl NamedManifest {
    pub fn resolve(self, declarations: &[Named], settings: &mut SchemaSettings) -> Result<Manifest, Error> {
        if self.provides.len() > ENTRY_LIMIT || self.requires.len() > ENTRY_LIMIT {
            return Err(Error::Budget);
        }
        let provides = self.provides.into_iter().map(|contract| contract.resolve(declarations, settings)).collect::<Result<_, _>>()?;
        let requires = self
            .requires
            .into_iter()
            .map(|entry| {
                Ok(Required {
                    contract: entry.contract.resolve(declarations, settings)?,
                    grants: entry.grants,
                })
            })
            .collect::<Result<_, Error>>()?;
        let resolve = |entry: NamedLifecycle| -> Result<lifecycle::Descriptor, Error> {
            if entry.identity != "harmos.operation.v1" {
                return Err(Error::Identity);
            }
            Ok(lifecycle::Descriptor::new(entry.action, entry.policy)?)
        };
        let [initialize, shutdown] = self.lifecycle;
        let lifecycle = [resolve(initialize)?, resolve(shutdown)?];
        let manifest = Manifest {
            facility: self.facility,
            owner: self.owner,
            provides,
            requires,
            lifecycle,
        };
        validate(&manifest)?;
        Ok(manifest)
    }
}

impl TryFrom<Decoded> for Manifest {
    type Error = Error;

    fn try_from(value: Decoded) -> Result<Self, Error> {
        if value.kind.as_deref().is_some_and(|kind| kind != "contracts") {
            return Err(Error::Kind);
        }
        let manifest = Self {
            facility: value.facility,
            owner: value.owner,
            provides: value.provides,
            requires: value.requires,
            lifecycle: value.lifecycle,
        };
        validate(&manifest)?;
        Ok(manifest)
    }
}

impl Required {
    pub fn from_tree(entries: &[Node<Requirement>]) -> Result<Vec<Self>, crate::operations::ContractError> {
        let mut required = Vec::with_capacity(crate::catalog::count(entries));
        crate::catalog::try_for_each(entries, |entry| {
            required.push(Self {
                contract: entry.declaration.contract()?,
                grants: entry.grants.to_vec(),
            });
            Ok::<(), crate::operations::ContractError>(())
        })?;
        Ok(required)
    }

    pub fn grants(&self, grant: &Grant) -> bool {
        self.grants.contains(grant)
    }
}

impl<const N: usize> Encoder<N> {
    pub(crate) const fn raw(&mut self, bytes: &[u8]) {
        if N == 0 {
            self.cursor += bytes.len();
        } else {
            super::write(&mut self.bytes, &mut self.cursor, bytes);
        }
    }

    pub(crate) const fn quoted(&mut self, value: &str) {
        if N == 0 {
            self.cursor += super::quoted_len(value);
        } else {
            super::quoted(&mut self.bytes, &mut self.cursor, value);
        }
    }

    pub(crate) const fn number(&mut self, value: u64) {
        if N == 0 {
            self.cursor += super::digits(value);
        } else {
            super::number(&mut self.bytes, &mut self.cursor, value);
        }
    }
}

pub fn decode(fragment: &[u8]) -> Result<Manifest, Error> {
    if fragment.len() > crate::FRAGMENT_LIMIT {
        return Err(Error::Budget);
    }
    Ok(serde_json::from_slice(fragment)?)
}

pub fn select(manifests: Vec<Manifest>, facility: &str, owner: &str) -> Result<Manifest, Error> {
    let mut matching = manifests.into_iter().filter(|value| value.facility == facility && value.owner == owner);
    let manifest = matching.next().ok_or_else(|| Error::Missing {
        facility: facility.to_owned(),
        owner: owner.to_owned(),
    })?;
    if matching.next().is_some() {
        return Err(Error::Duplicate {
            facility: facility.to_owned(),
            owner: owner.to_owned(),
        });
    }
    validate(&manifest)?;
    Ok(manifest)
}

pub fn validate(manifest: &Manifest) -> Result<(), Error> {
    if !valid_identity(&manifest.facility, &manifest.owner) {
        return Err(Error::Identity);
    }
    let required = manifest.requires.iter().map(|required| &required.contract);
    for (direction, contracts) in [("provides", manifest.provides.iter().collect::<Vec<_>>()), ("requires", required.collect())] {
        if contracts.len() > ENTRY_LIMIT {
            return Err(Error::Budget);
        }
        let mut ids = BTreeSet::new();
        for contract in contracts {
            contract.operation().validate()?;
            if !ids.insert(&contract.operation().id) {
                return Err(Error::OperationDuplicate {
                    direction,
                    id: contract.operation().id.to_string(),
                });
            }
        }
    }
    let grants = manifest.requires.iter().map(|required| required.grants.len()).sum::<usize>();
    if grants > ENTRY_LIMIT {
        return Err(Error::Budget);
    }
    for required in &manifest.requires {
        let mut seen = BTreeSet::new();
        let operation = &required.contract.operation().id;
        for grant in &required.grants {
            if grant.operation() != operation.as_str()
                || grant.scope().is_none_or(|scope| scope.is_empty() || scope.len() > GRANT_LIMIT)
                || !seen.insert(grant)
            {
                return Err(Error::Grant {
                    id: required.contract.operation().id.to_string(),
                });
            }
        }
    }
    if manifest.lifecycle[0].action() != lifecycle::Action::Initialize || manifest.lifecycle[1].action() != lifecycle::Action::Shutdown {
        return Err(Error::LifecycleOrder);
    }
    Ok(())
}

pub const fn encoded_tree_len(
    facility: &str, owner: &str, provides: &[Node<WireDeclaration>], requires: &[Node<Requirement>], lifecycle: &[OperationPolicy; 2],
) -> usize {
    let size = encoded::<0>(facility, owner, provides, requires, lifecycle).cursor;
    assert!(size - PREFIX.len() - 1 <= crate::FRAGMENT_LIMIT);
    size
}

pub const fn encode_tree<const N: usize>(
    facility: &str, owner: &str, provides: &[Node<WireDeclaration>], requires: &[Node<Requirement>], lifecycle: &[OperationPolicy; 2],
) -> [u8; N] {
    assert!(N == encoded_tree_len(facility, owner, provides, requires, lifecycle));
    encoded::<N>(facility, owner, provides, requires, lifecycle).bytes
}

pub const fn identity_len(facility: &str, id: &str, version: &str, description: &str, config: &str) -> usize {
    let size = encoded_identity::<0>(facility, id, version, description, config).cursor;
    assert!(size - PREFIX.len() - 1 <= crate::FRAGMENT_LIMIT);
    size
}

pub const fn encode_identity<const N: usize>(facility: &str, id: &str, version: &str, description: &str, config: &str) -> [u8; N] {
    assert!(N == identity_len(facility, id, version, description, config));
    encoded_identity::<N>(facility, id, version, description, config).bytes
}

pub(crate) const fn valid_identity(facility: &str, owner: &str) -> bool {
    matches!(facility.as_bytes(), b"guest" | b"sidecar") && crate::identity::segment(owner)
}

pub(crate) const fn wire<const N: usize>(out: &mut Encoder<N>, entry: &WireDeclaration) {
    out.raw(b"{\"operation\":");
    declaration(out, &entry.operation);
    out.raw(b",\"identity\":\"harmos.operation.v1\",\"payload_codec\":");
    out.quoted(entry.codec);
    out.raw(b",\"response_codec\":");
    out.quoted(entry.response_codec);
    if let Some(codec) = entry.input_codec {
        out.raw(b",\"input_codec\":");
        out.quoted(codec);
    }
    if let Some(codec) = entry.output_codec {
        out.raw(b",\"output_codec\":");
        out.quoted(codec);
    }
    out.raw(b",\"codec\":\"postcard\",\"request\":");
    out.quoted(entry.request.name);
    out.raw(b",\"response\":");
    out.quoted(entry.response.name);
    if let Some(input) = entry.input {
        out.raw(b",\"input\":");
        out.quoted(input.name);
    }
    if let Some(output) = entry.output {
        out.raw(b",\"output\":");
        out.quoted(output.name);
    }
    if let Some(binding) = &entry.grpc {
        out.raw(b",\"grpc\":{\"package\":");
        out.quoted(match &binding.package {
            Cow::Borrowed(value) => value,
            Cow::Owned(_) => panic!("const gRPC package must be borrowed"),
        });
        out.raw(b",\"service\":");
        out.quoted(match &binding.service {
            Cow::Borrowed(value) => value,
            Cow::Owned(_) => panic!("const gRPC service must be borrowed"),
        });
        out.raw(b",\"method\":");
        out.quoted(match &binding.method {
            Cow::Borrowed(value) => value,
            Cow::Owned(_) => panic!("const gRPC method must be borrowed"),
        });
        out.raw(b"}");
    }
    out.raw(b"}");
}

pub(crate) const fn policy<const N: usize>(out: &mut Encoder<N>, value: OperationPolicy) {
    out.raw(b"{\"timeout\":");
    duration(out, value.timeout);
    out.raw(b",\"attempt_timeout\":");
    duration(out, value.attempt_timeout);
    out.raw(b",\"retries\":");
    match value.retries {
        RetryPolicy::Never => out.quoted("never"),
        RetryPolicy::OnTransient { max_retries, backoff: value } => {
            out.raw(b"{\"on_transient\":{\"max_retries\":");
            out.number(max_retries as u64);
            out.raw(b",\"backoff\":");
            backoff(out, value);
            out.raw(b"}}");
        }
    }
    out.raw(b"}");
}

const fn encoded_identity<const N: usize>(facility: &str, id: &str, version: &str, description: &str, config: &str) -> Encoder<N> {
    assert!(valid_identity(facility, id));
    let mut out = Encoder { bytes: [0; N], cursor: 0 };
    out.raw(PREFIX);
    out.raw(b"{\"kind\":\"plugin\",\"facility\":");
    out.quoted(facility);
    out.raw(b",\"id\":");
    out.quoted(id);
    out.raw(b",\"version\":");
    out.quoted(version);
    out.raw(b",\"description\":");
    out.quoted(description);
    out.raw(b",\"config\":");
    out.quoted(config);
    out.raw(b"}\0");
    out
}

const fn encoded<const N: usize>(
    facility: &str, owner: &str, provides: &[Node<WireDeclaration>], requires: &[Node<Requirement>], lifecycle: &[OperationPolicy; 2],
) -> Encoder<N> {
    assert!(valid_identity(facility, owner));
    assert!(crate::catalog::count(provides) <= ENTRY_LIMIT);
    assert!(crate::catalog::count(requires) <= ENTRY_LIMIT);
    let mut out = Encoder { bytes: [0; N], cursor: 0 };
    out.raw(PREFIX);
    out.raw(b"{\"kind\":\"contracts\",\"facility\":");
    out.quoted(facility);
    out.raw(b",\"owner\":");
    out.quoted(owner);
    out.raw(b",\"provides\":[");
    tree(&mut out, provides, &mut true);
    out.raw(b"],\"requires\":[");
    required(&mut out, requires, &mut true);
    out.raw(b"],\"lifecycle\":[{\"action\":\"initialize\",\"identity\":\"harmos.operation.v1\",\"policy\":");
    policy(&mut out, lifecycle[0]);
    out.raw(b"},{\"action\":\"shutdown\",\"identity\":\"harmos.operation.v1\",\"policy\":");
    policy(&mut out, lifecycle[1]);
    out.raw(b"}]}\0");
    out
}

const fn required<const N: usize>(out: &mut Encoder<N>, entries: &[Node<Requirement>], first: &mut bool) {
    let mut index = 0;
    while index < entries.len() {
        match &entries[index] {
            Node::Leaf(entry) => {
                if !*first {
                    out.raw(b",");
                }
                *first = false;
                out.raw(b"{\"contract\":");
                wire(out, &entry.declaration);
                out.raw(b",\"grants\":[");
                let mut grant = 0;
                while grant < entry.grants.len() {
                    if grant > 0 {
                        out.raw(b",");
                    }
                    let (operation, scope) = entry.grants[grant].borrowed();
                    assert!(
                        super::publications::same_bytes(operation, entry.declaration.operation.id),
                        "a requirement grants only its own operation"
                    );
                    let Some(scope) = scope else {
                        panic!("a requirement grant names its scope");
                    };
                    assert!(!scope.is_empty() && scope.len() <= GRANT_LIMIT, "grant scopes are nonempty and bounded");
                    out.raw(b"{\"operation\":");
                    out.quoted(operation);
                    out.raw(b",\"scope\":");
                    out.quoted(scope);
                    out.raw(b"}");
                    grant += 1;
                }
                out.raw(b"]}");
            }
            Node::Nested(nested) => required(out, nested, first),
        }
        index += 1;
    }
}

const fn tree<const N: usize>(out: &mut Encoder<N>, entries: &[Node<WireDeclaration>], first: &mut bool) {
    let mut index = 0;
    while index < entries.len() {
        match &entries[index] {
            Node::Leaf(entry) => {
                if !*first {
                    out.raw(b",");
                }
                *first = false;
                wire(out, entry);
            }
            Node::Nested(nested) => tree(out, nested, first),
        }
        index += 1;
    }
}

const fn declaration<const N: usize>(out: &mut Encoder<N>, entry: &Entry) {
    out.raw(b"{\"id\":");
    out.quoted(entry.id);
    out.raw(b",\"description\":");
    out.quoted(entry.description);
    out.raw(b",\"policy\":");
    policy(out, entry.policy);
    out.raw(b",\"visibility\":");
    out.quoted(match entry.visibility {
        Visibility::Private => "private",
        Visibility::Public => "public",
        Visibility::Host => "host",
    });
    out.raw(b",\"replay\":");
    out.quoted(match entry.replay {
        ReplaySafety::Unknown => "unknown",
        ReplaySafety::Idempotent => "idempotent",
        ReplaySafety::Deduplicated => "deduplicated",
    });
    out.raw(b"}");
}

const fn duration<const N: usize>(out: &mut Encoder<N>, value: Duration) {
    out.raw(b"{\"secs\":");
    out.number(value.as_secs());
    out.raw(b",\"nanos\":");
    out.number(value.subsec_nanos() as u64);
    out.raw(b"}");
}

const fn backoff<const N: usize>(out: &mut Encoder<N>, value: Backoff) {
    let jitter = match value {
        Backoff::Fixed { delay, jitter } => {
            out.raw(b"{\"fixed\":{\"delay\":");
            duration(out, delay);
            jitter
        }
        Backoff::Exponential { initial, maximum, jitter } => {
            out.raw(b"{\"exponential\":{\"initial\":");
            duration(out, initial);
            out.raw(b",\"maximum\":");
            duration(out, maximum);
            jitter
        }
    };
    out.raw(b",\"jitter\":");
    out.quoted(match jitter {
        Jitter::None => "none",
        Jitter::Deterministic => "deterministic",
    });
    out.raw(b"}}");
}
