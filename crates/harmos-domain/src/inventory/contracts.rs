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

use crate::model::operations::lifecycle;

// Standard Library Imports
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::time::Duration;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::PREFIX;
use crate::catalog::Node;
use crate::config::SchemaSettings;
use crate::operations::{Backoff, Contract, Entry, Jitter, OperationPolicy, ReplaySafety, RetryPolicy, Visibility, WireDeclaration};
use crate::plugin::{Grant, Requirement};
use crate::schema::declaration::Named;
use crate::schema::graph::Schema;

/* === Constants === */

const ENTRY_LIMIT: usize = 4096;
const GRANT_LIMIT: usize = 256;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

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
pub struct NamedFragment {
    pub facility: String,
    pub owner: String,
    pub entry: NamedEntry,
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

/* === Enums === */

#[derive(Deserialize)]
#[serde(tag = "direction", rename_all = "snake_case", deny_unknown_fields)]
pub enum NamedEntry {
    Provides { contract: NamedContract },
    Requires { contract: NamedContract, grants: Vec<Grant> },
    Lifecycle { lifecycle: [NamedLifecycle; 2] },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing complete {facility} contract inventory for `{owner}`")]
    Missing { facility: String, owner: String },

    #[error("multiple {facility} contract lifecycle headers for `{owner}`")]
    Duplicate { facility: String, owner: String },

    #[error("contract inventory exceeds its entry or fragment budget")]
    Budget,

    #[error("contract inventory requires a guest or sidecar facility and a valid artifact owner")]
    Identity,

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
}

/* === Traits === */

/* === Implementations === */

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

impl TryFrom<Decoded> for Manifest {
    type Error = Error;

    fn try_from(value: Decoded) -> Result<Self, Error> {
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

/* === Public Functions === */

pub fn select(
    fragments: Vec<NamedFragment>, facility: &str, owner: &str, declarations: &[Named], settings: &mut SchemaSettings,
) -> Result<Manifest, Error> {
    let mut provides = Vec::new();
    let mut requires = Vec::new();
    let mut lifecycle = None;
    for fragment in fragments {
        if !valid_identity(&fragment.facility, &fragment.owner) {
            return Err(Error::Identity);
        }
        if fragment.facility != facility || fragment.owner != owner {
            continue;
        }
        match fragment.entry {
            NamedEntry::Provides { contract } => {
                if provides.len() == ENTRY_LIMIT {
                    return Err(Error::Budget);
                }
                provides.push(contract.resolve(declarations, settings)?);
            }
            NamedEntry::Requires { contract, grants } => {
                if requires.len() == ENTRY_LIMIT {
                    return Err(Error::Budget);
                }
                requires.push(Required {
                    contract: contract.resolve(declarations, settings)?,
                    grants,
                });
            }
            NamedEntry::Lifecycle { lifecycle: entries } => {
                if lifecycle.is_some() {
                    return Err(Error::Duplicate {
                        facility: facility.to_owned(),
                        owner: owner.to_owned(),
                    });
                }
                let resolve = |entry: NamedLifecycle| -> Result<lifecycle::Descriptor, Error> {
                    if entry.identity != "harmos.operation.v1" {
                        return Err(Error::Identity);
                    }
                    Ok(lifecycle::Descriptor::new(entry.action, entry.policy)?)
                };
                let [initialize, shutdown] = entries;
                lifecycle = Some([resolve(initialize)?, resolve(shutdown)?]);
            }
        }
    }
    let lifecycle = lifecycle.ok_or_else(|| Error::Missing {
        facility: facility.to_owned(),
        owner: owner.to_owned(),
    })?;
    let manifest = Manifest {
        facility: facility.to_owned(),
        owner: owner.to_owned(),
        provides,
        requires,
        lifecycle,
    };
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

pub const fn encoded_fragments_len(
    facility: &str, owner: &str, provides: &[Node<WireDeclaration>], requires: &[Node<Requirement>], lifecycle: &[OperationPolicy; 2],
) -> usize {
    encoded::<0>(facility, owner, provides, requires, lifecycle).cursor
}

pub const fn encode_fragments<const N: usize>(
    facility: &str, owner: &str, provides: &[Node<WireDeclaration>], requires: &[Node<Requirement>], lifecycle: &[OperationPolicy; 2],
) -> [u8; N] {
    assert!(N == encoded_fragments_len(facility, owner, provides, requires, lifecycle));
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
    out.raw(b",\"cancellation_timeout\":");
    duration(out, value.cancellation_timeout);
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

/* === Private Functions === */

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
    let start = begin(&mut out, facility, owner, "lifecycle");
    out.raw(b",\"lifecycle\":[{\"action\":\"initialize\",\"identity\":\"harmos.operation.v1\",\"policy\":");
    policy(&mut out, lifecycle[0]);
    out.raw(b"},{\"action\":\"shutdown\",\"identity\":\"harmos.operation.v1\",\"policy\":");
    policy(&mut out, lifecycle[1]);
    out.raw(b"}]");
    finish(&mut out, start);
    tree(&mut out, facility, owner, provides);
    required(&mut out, facility, owner, requires);
    out
}

const fn begin<const N: usize>(out: &mut Encoder<N>, facility: &str, owner: &str, direction: &str) -> usize {
    out.raw(PREFIX);
    let start = out.cursor;
    out.raw(b"{\"kind\":\"contracts\",\"facility\":");
    out.quoted(facility);
    out.raw(b",\"owner\":");
    out.quoted(owner);
    out.raw(b",\"entry\":{\"direction\":");
    out.quoted(direction);
    start
}

const fn finish<const N: usize>(out: &mut Encoder<N>, start: usize) {
    out.raw(b"}}");
    assert!(out.cursor - start <= crate::FRAGMENT_LIMIT);
    out.raw(b"\0");
}

const fn required<const N: usize>(out: &mut Encoder<N>, facility: &str, owner: &str, entries: &[Node<Requirement>]) {
    let mut index = 0;
    while index < entries.len() {
        match &entries[index] {
            Node::Leaf(entry) => {
                let start = begin(out, facility, owner, "requires");
                out.raw(b",\"contract\":");
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
                out.raw(b"]");
                finish(out, start);
            }
            Node::Nested(nested) => required(out, facility, owner, nested),
        }
        index += 1;
    }
}

const fn tree<const N: usize>(out: &mut Encoder<N>, facility: &str, owner: &str, entries: &[Node<WireDeclaration>]) {
    let mut index = 0;
    while index < entries.len() {
        match &entries[index] {
            Node::Leaf(entry) => {
                let start = begin(out, facility, owner, "provides");
                out.raw(b",\"contract\":");
                wire(out, entry);
                finish(out, start);
            }
            Node::Nested(nested) => tree(out, facility, owner, nested),
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

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{Declaration, Facility};
    use crate::operations::{NoStream, OperationDeclaration};
    use serde_json::{Value, json};

    struct Read;

    impl OperationDeclaration for Read {
        type Request = ();
        type Response = ();
        type Input = NoStream;
        type Output = NoStream;
        const ID: &'static str = "fixture.read";
        const POLICY: OperationPolicy = lifecycle::POLICY;
    }

    const SDK: [OperationPolicy; 2] = [lifecycle::POLICY; 2];
    const DESCRIPTION: &str = match str::from_utf8(&[b'x'; 9000]) {
        Ok(value) => value,
        Err(_) => panic!("ASCII"),
    };
    const IDS: [&str; 4] = ["fixture.one", "fixture.two", "fixture.three", "fixture.four"];
    const PROVIDED: &[Node<WireDeclaration>] = &{
        let mut entries = [const { Node::Leaf(WireDeclaration::of::<Read>()) }; 4];
        let mut index = 0;
        while index < entries.len() {
            let Node::Leaf(entry) = &mut entries[index] else {
                panic!("leaf");
            };
            entry.operation.id = IDS[index];
            entry.operation.description = DESCRIPTION;
            index += 1;
        }
        entries
    };
    const REQUIRED: &[Node<Requirement>] = &{
        let mut entries = [const {
            Node::Leaf(Requirement {
                declaration: WireDeclaration::of::<Read>(),
                grants: &[],
            })
        }; 4];
        let mut index = 0;
        while index < entries.len() {
            let Node::Leaf(entry) = &mut entries[index] else {
                panic!("leaf");
            };
            entry.declaration.operation.id = IDS[index];
            entry.declaration.operation.description = DESCRIPTION;
            index += 1;
        }
        entries
    };
    const PROVIDED_TREE: &[Node<WireDeclaration>] = &[Node::Nested(PROVIDED)];
    const REQUIRED_TREE: &[Node<Requirement>] = &[Node::Nested(REQUIRED)];
    const SIZE: usize = encoded_fragments_len("sidecar", "fixture", PROVIDED_TREE, REQUIRED_TREE, &SDK);
    static CONTRACTS: [u8; SIZE] = encode_fragments("sidecar", "fixture", PROVIDED_TREE, REQUIRED_TREE, &SDK);

    fn parts() -> Vec<Value> {
        crate::artifacts::fragments(&CONTRACTS, &["contracts"]).unwrap()
    }

    fn bytes(parts: &[Value]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for part in parts {
            bytes.extend_from_slice(PREFIX);
            bytes.extend(serde_json::to_vec(part).unwrap());
            bytes.push(0);
        }
        bytes
    }

    fn artifact(facility: &str, parts: &mut [Value]) -> Vec<u8> {
        for part in parts.iter_mut() {
            part["facility"] = facility.into();
        }
        let mut artifact = bytes(parts);
        artifact.extend(bytes(&[
            json!({"kind":"plugin","facility":facility,"id":"fixture","version":"1.0.0","description":"","config":"()"}),
            json!({"kind":"publications","facility":facility,"owner":"fixture","streams":[]}),
            json!({"kind":"observers","facility":facility,"owner":"fixture","observers":[]}),
        ]));
        let schemas =
            crate::schema::declaration::markers_many([WireDeclaration::of::<Read>().request], &SchemaSettings::default()).unwrap();
        for schema in schemas {
            artifact.extend_from_slice(schema);
        }
        artifact
    }

    #[test]
    fn encoded_inventory_round_trip() {
        let mut artifact = artifact("sidecar", &mut []);
        artifact.extend_from_slice(&CONTRACTS);
        let declaration = Declaration::inspect(&artifact, Facility::Sidecar).unwrap();
        assert_eq!(declaration.contracts.provides.len(), PROVIDED.len());
        assert_eq!(declaration.contracts.requires.len(), REQUIRED.len());
        assert_eq!(declaration.contracts.lifecycle[0], lifecycle::Descriptor::new(lifecycle::Action::Initialize, SDK[0]).unwrap());
        assert_eq!(declaration.contracts.lifecycle[1], lifecycle::Descriptor::new(lifecycle::Action::Shutdown, SDK[1]).unwrap());
    }

    #[test]
    fn total_over_fragment_limit_packages_and_resolves() {
        use harmos_packaging::bundle::{Bundle, Executable, Facility as Packaged, Identity, Limits, Source};
        assert!(CONTRACTS.len() > crate::FRAGMENT_LIMIT);
        let parts = parts();
        assert_eq!(parts.len(), 1 + PROVIDED.len() + REQUIRED.len());
        assert!(parts.iter().all(|part| serde_json::to_vec(part).unwrap().len() <= crate::FRAGMENT_LIMIT));
        for (facility, inspected, packaged) in
            [("sidecar", Facility::Sidecar, Packaged::Sidecar), ("guest", Facility::Guest, Packaged::Guest)]
        {
            let artifact = artifact(facility, &mut parts.clone());
            let declaration = Declaration::inspect(&artifact, inspected).unwrap();
            assert_eq!(declaration.contracts.provides.len(), 4);
            assert_eq!(declaration.contracts.requires.len(), 4);
            for (provided, required) in declaration.contracts.provides.iter().zip(&declaration.contracts.requires) {
                assert_eq!(provided, &required.contract);
                assert_eq!(provided.operation().description, DESCRIPTION);
            }
            let identity = Identity {
                name: "fixture".into(),
                version: "1.0.0".parse().unwrap(),
                description: String::new(),
                package: Default::default(),
            };
            let executable = Executable {
                facility: packaged,
                target: "fixture-target".into(),
                registrations: Vec::new(),
            };
            let source = Source {
                path: "plugin.bin",
                bytes: &artifact,
                executable: Some(executable),
            };
            let bundle = Bundle::build(identity, &[source], Limits::default()).unwrap();
            let reopened = Bundle::inspect(bundle.bytes(), Limits::default()).unwrap();
            let packaged = reopened.declaration("plugin.bin").unwrap();
            assert_eq!(packaged.id, declaration.id);
            assert_eq!(serde_json::to_value(&packaged.contracts).unwrap(), serde_json::to_value(&declaration.contracts).unwrap());
        }
    }

    #[test]
    fn duplicate_entries_across_fragments_are_refused() {
        for direction in ["provides", "requires"] {
            let mut parts = parts();
            let duplicate = parts.iter().find(|part| part["entry"]["direction"] == direction).unwrap().clone();
            parts.push(duplicate);
            let error = Declaration::inspect(&artifact("sidecar", &mut parts), Facility::Sidecar).unwrap_err();
            assert!(
                matches!(error, crate::inventory::DeclarationError::ContractInventory(Error::OperationDuplicate { direction: actual, .. }) if actual == direction)
            );
        }
    }

    #[test]
    fn lifecycle_header_is_required_and_unique() {
        let mut duplicate = parts();
        duplicate.push(duplicate[0].clone());
        assert!(matches!(
            Declaration::inspect(&artifact("sidecar", &mut duplicate), Facility::Sidecar),
            Err(crate::inventory::DeclarationError::ContractInventory(Error::Duplicate { .. }))
        ));
        let mut missing = parts();
        missing.remove(0);
        assert!(matches!(
            Declaration::inspect(&artifact("sidecar", &mut missing), Facility::Sidecar),
            Err(crate::inventory::DeclarationError::ContractInventory(Error::Missing { .. }))
        ));
    }

    #[test]
    fn merged_entry_limit_is_enforced() {
        for direction in ["provides", "requires"] {
            let template = parts().into_iter().find(|part| part["entry"]["direction"] == direction).unwrap();
            let mut parts = vec![parts().remove(0)];
            for index in 0..=ENTRY_LIMIT {
                let mut part = template.clone();
                part["entry"]["contract"]["operation"]["id"] = format!("fixture.operation{index}").into();
                part["entry"]["contract"]["operation"]["description"] = "".into();
                parts.push(part);
            }
            let fragments = parts
                .into_iter()
                .map(|mut part| {
                    part.as_object_mut().unwrap().remove("kind");
                    serde_json::from_value(part).unwrap()
                })
                .collect();
            let mut settings = SchemaSettings {
                preparation_bytes: std::num::NonZeroUsize::new(usize::MAX).unwrap(),
                preparation_work: std::num::NonZeroUsize::new(usize::MAX).unwrap(),
            };
            let markers = crate::schema::declaration::markers_many([WireDeclaration::of::<Read>().request], &settings).unwrap();
            let schemas = crate::artifacts::fragments::<Named>(&markers.concat(), &["schema"]).unwrap();
            assert!(matches!(select(fragments, "sidecar", "fixture", &schemas, &mut settings), Err(Error::Budget)));
        }
    }

    #[test]
    fn aggregate_shape_is_refused() {
        let mut parts = parts();
        parts[0] = json!({"kind":"contracts","facility":"sidecar","owner":"fixture","provides":[],"requires":[],"lifecycle":parts[0]["entry"]["lifecycle"]});
        assert!(matches!(
            Declaration::inspect(&artifact("sidecar", &mut parts), Facility::Sidecar),
            Err(crate::inventory::DeclarationError::Malformed { .. })
        ));
    }

    #[test]
    fn aggregate_preparation_budgets_remain_enforced() {
        let count = parts().len();
        for memory in [false, true] {
            let mut settings = SchemaSettings::default();
            if memory {
                settings.preparation_bytes = std::num::NonZeroUsize::new(CONTRACTS.len()).unwrap();
            } else {
                settings.preparation_work = std::num::NonZeroUsize::new(CONTRACTS.len() / 2).unwrap();
            }
            assert!(matches!(
                crate::artifacts::fragments_with_settings::<Value>(&CONTRACTS, &["contracts"], &mut settings),
                Err(crate::ScanError::Budget)
            ));
        }
        assert_eq!(count, 9);
    }

    #[test]
    fn individual_fragment_limit_remains_enforced() {
        let oversized = vec![b'x'; crate::FRAGMENT_LIMIT + 1];
        let mut artifact = PREFIX.to_vec();
        artifact.extend(oversized);
        artifact.push(0);
        assert!(matches!(crate::artifacts::fragments::<Value>(&artifact, &["contracts"]), Err(crate::ScanError::Oversized)));
        let mut provided = WireDeclaration::of::<Read>();
        provided.operation.description = str::from_utf8(&[b'x'; crate::FRAGMENT_LIMIT]).unwrap();
        assert!(std::panic::catch_unwind(|| encoded_fragments_len("sidecar", "fixture", &[Node::Leaf(provided)], &[], &SDK)).is_err());
    }
}
