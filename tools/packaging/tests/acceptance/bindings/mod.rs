/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Export Generated Python Packages and Consume Them from the Repository
// Interpreter. Prerequisite: `uv sync --locked --project tools/python --group test`
// provisions the root `.venv` with protobuf, build and ty.

/* === Modules === */

mod aggregate_members_typed;
mod export_is_deterministic;
mod export_refuses_invalid;
mod exported_bindings_support_every_consumer;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;

// Third Party Imports

// Local Imports
use crate::fixtures;
use harmos_domain::config::SchemaSettings;
use harmos_domain::schema::{
    Cardinality, Described,
    graph::{Field, Kind, Schema},
};
use harmos_domain::{Aggregate, Identifier, QualifiedId};
use harmos_operations::execution::grpc::{self, DecodeInput};
use harmos_operations::operations::{Codec, Contract, Grpc, OperationPolicy, RetryPolicy};
use harmos_packaging::{
    proto::Artifacts,
    python::{self, Settings},
};

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    timeout: std::time::Duration::from_secs(1),
    attempt_timeout: std::time::Duration::from_secs(1),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

// A Wide Schema Shared with the Domain's Graph Stories
const CHECKPOINT: &str = include_str!("../../../../../crates/harmos-domain/tests/fixtures/checkpoint-schema.json");

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug, Default)]
struct Payload {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[derive(harmos_domain::Message, Debug, Default)]
#[harmos_path(harmos_operations)]
#[derive(harmos_operations::Request)]
#[request(response = Payload)]
#[derive(Clone)]
struct ReadOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

static READ_OPERATION_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<ReadOperation>("read", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .grpc(harmos_operations::operations::Grpc::new("test.v1", "Shapes", "Read"));

const READ_OPERATION_OPERATION: harmos_operations::operations::OperationSpec<ReadOperation> =
    harmos_operations::operations::OperationSpec::<ReadOperation>::new(&READ_OPERATION_DESCRIPTION);

#[derive(harmos_domain::Message, Debug, Default)]
#[harmos_path(harmos_operations)]
#[derive(harmos_operations::Request)]
#[request(response = Payload, input = Payload, output = Payload)]
#[derive(Clone)]
struct ExchangeOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

static EXCHANGE_OPERATION_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<ExchangeOperation>("exchange", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .grpc(harmos_operations::operations::Grpc::new("test.v1", "Shapes", "Exchange"));

const EXCHANGE_OPERATION_OPERATION: harmos_operations::operations::OperationSpec<ExchangeOperation> =
    harmos_operations::operations::OperationSpec::<ExchangeOperation>::new(&EXCHANGE_OPERATION_DESCRIPTION);

#[derive(harmos_domain::Message, Debug, Default)]
#[harmos_path(harmos_operations)]
#[derive(harmos_operations::Request)]
#[request(response = Payload, input = Payload)]
#[derive(Clone)]
struct UploadOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

static UPLOAD_OPERATION_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<UploadOperation>("upload", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .grpc(harmos_operations::operations::Grpc::new("test.v1", "Shapes", "Upload"));

const UPLOAD_OPERATION_OPERATION: harmos_operations::operations::OperationSpec<UploadOperation> =
    harmos_operations::operations::OperationSpec::<UploadOperation>::new(&UPLOAD_OPERATION_DESCRIPTION);

#[derive(harmos_domain::Message, Debug, Default)]
#[harmos_path(harmos_operations)]
#[derive(harmos_operations::Request)]
#[request(response = Payload, output = Payload)]
#[derive(Clone)]
struct DownloadOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

static DOWNLOAD_OPERATION_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<DownloadOperation>("download", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .grpc(harmos_operations::operations::Grpc::new("test.v1", "Shapes", "Download"));

const DOWNLOAD_OPERATION_OPERATION: harmos_operations::operations::OperationSpec<DownloadOperation> =
    harmos_operations::operations::OperationSpec::<DownloadOperation>::new(&DOWNLOAD_OPERATION_DESCRIPTION);

#[derive(harmos_domain::Message, Clone, Debug, PartialEq)]
struct Summary {
    aggregate: Aggregate,
    aggregates: Vec<Aggregate>,
}

// Two Producers Exported from One Catalog, Each in Its Own Directory
struct Exported {
    target: tempfile::TempDir,
    alpha: PathBuf,
    beta: PathBuf,
}

/* === Enums === */

static ROUTES_CATALOG_DECLARATIONS: [harmos_operations::catalog::Node<harmos_operations::operations::WireDeclaration>; 4] = [
    harmos_operations::catalog::Node::Leaf(READ_OPERATION_OPERATION.wire()),
    harmos_operations::catalog::Node::Leaf(EXCHANGE_OPERATION_OPERATION.wire()),
    harmos_operations::catalog::Node::Leaf(UPLOAD_OPERATION_OPERATION.wire()),
    harmos_operations::catalog::Node::Leaf(DOWNLOAD_OPERATION_OPERATION.wire()),
];
static ROUTES_CATALOG_CALLEES: [harmos_operations::catalog::Node<&'static [harmos_operations::operations::WireDeclaration]>; 4] = [
    harmos_operations::catalog::Node::Leaf(READ_OPERATION_OPERATION.callees()),
    harmos_operations::catalog::Node::Leaf(EXCHANGE_OPERATION_OPERATION.callees()),
    harmos_operations::catalog::Node::Leaf(UPLOAD_OPERATION_OPERATION.callees()),
    harmos_operations::catalog::Node::Leaf(DOWNLOAD_OPERATION_OPERATION.callees()),
];

const ROUTES_CATALOG: harmos_operations::execution::operations::CatalogDescription =
    harmos_operations::execution::operations::CatalogDescription::new(
        concat!(module_path!(), "::Routes"),
        &ROUTES_CATALOG_DECLARATIONS,
        &ROUTES_CATALOG_CALLEES,
        None,
    );

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

// Four Streaming Shapes Plus a Wide, a Recursive and a Reserved-Name Schema
fn contracts() -> Vec<Contract> {
    let mut contracts = harmos_operations::operations::Contracts::from_tree(ROUTES_CATALOG.declarations).unwrap();
    let template = contracts[0].clone();
    let checkpoint: Schema = serde_json::from_str(CHECKPOINT).unwrap();
    let recursive = Schema {
        representations: ::std::collections::BTreeMap::new(),
        root: 0,
        definitions: vec![vec![Field {
            name: Cow::Borrowed("next"),
            tag: 1,
            cardinality: Cardinality::Optional,
            kind: Kind::Message { definition: 0 },
        }]],
    };
    contracts.push(contract(&template, "fixture_checkpoint", "Checkpoint", checkpoint));
    contracts.push(contract(&template, "fixture_recursive", "Recursive", recursive));
    contracts.push(contract(&template, "fixture_reserved", "Reserved", reserved()));

    contracts
}

fn export() -> Exported {
    let contracts = contracts();
    let artifacts = Artifacts::new(&contracts).unwrap();
    let target = tempfile::tempdir().unwrap();
    let alpha = target.path().join("alpha");
    let beta = target.path().join("beta");
    python::generate(
        python::Catalog {
            contracts: &artifacts.operation_contracts().unwrap(),
            settings: &settings("bindings_alpha"),
            protoc: &protoc(),
        },
        &alpha,
    )
    .unwrap();
    python::generate(
        python::Catalog {
            contracts: &artifacts.operation_contracts().unwrap(),
            settings: &settings("bindings_beta"),
            protoc: &protoc(),
        },
        &beta,
    )
    .unwrap();

    Exported { target, alpha, beta }
}

fn reserved() -> Schema {
    Schema {
        representations: ::std::collections::BTreeMap::new(),
        root: 0,
        definitions: vec![
            vec![Field {
                name: Cow::Borrowed("Clear"),
                tag: 7,
                cardinality: Cardinality::Variant,
                kind: Kind::Message { definition: 1 },
            }],
            vec![],
        ],
    }
}

fn settings(module: &str) -> Settings {
    Settings {
        distribution: module.replace('_', "-"),
        module: module.into(),
        version: "1.2.3-rc.4".parse().unwrap(),
        description: "Generated Boundary Fixture".into(),
        license: "Apache-2.0".into(),
        operations: Vec::new(),
    }
}

// Wrap a Direction Schema in the Request and Response Root of a New Contract
fn contract(template: &Contract, id: &str, method: &'static str, mut schema: Schema) -> Contract {
    let mut operation = template.operation().clone();
    operation.id = Identifier::new(id).unwrap();
    let direction = schema.root;
    let root = ["request", "response"]
        .into_iter()
        .zip(1..)
        .map(|(name, tag)| Field {
            name: Cow::Borrowed(name),
            tag,
            cardinality: Cardinality::Required,
            kind: Kind::Message { definition: direction },
        })
        .collect();
    schema.root = schema.definitions.len();
    schema.definitions.push(root);

    {
        let fingerprint = harmos_domain::schema::identity::contract(id, &schema, "harmos.protobuf.v1").unwrap();
        operation.fingerprint = fingerprint;
        Contract::new(operation, fingerprint, Codec::Protobuf, schema)
    }
    .unwrap()
    .with_grpc(Some(Grpc::new("test.v1", "Shapes", method)))
    .unwrap()
}

fn protoc() -> PathBuf {
    fixtures::protoc()
}

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut directories = vec![root.to_owned()];
    let mut result = BTreeMap::new();
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                directories.push(entry.path());
                continue;
            }
            let relative = entry.path().strip_prefix(root).unwrap().to_owned();
            result.insert(relative, fs::read(entry.path()).unwrap());
        }
    }

    result
}

// Run One Consumer Scene; `pure` Selects The Pure-Python Protobuf Runtime
fn scene(name: &str, arguments: &[&Path], pure: bool, input: &[u8]) -> Vec<u8> {
    let repository = fixtures::workspace();
    let interpreter = repository.join(".venv").join(if cfg!(windows) { "Scripts/python.exe" } else { "bin/python" });
    assert!(interpreter.is_file(), "{} is missing; run `uv sync --locked --project tools/python --group test`", interpreter.display());
    let mut command = fixtures::command(interpreter);
    command
        .arg(fixtures::manifest().join("tests/acceptance/bindings.py"))
        .arg(name)
        .args(arguments)
        .env_remove("PROTOCOL_BUFFERS_PYTHON_IMPLEMENTATION")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if pure {
        command.env("PROTOCOL_BUFFERS_PYTHON_IMPLEMENTATION", "python");
    }

    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "scene {name}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    output.stdout
}

fn both(name: &str, exported: &Exported, input: &[u8]) -> Vec<Vec<u8>> {
    let arguments = [exported.target.path(), &exported.alpha, &exported.beta];

    [false, true].into_iter().map(|pure| scene(name, &arguments, pure, input)).collect()
}

/* === Tests === */
