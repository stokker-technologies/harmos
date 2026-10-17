/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Export Generated Python Packages And Consume Them From The Repository
// Interpreter. Prerequisite: `uv sync --locked --project tools/python --group test`
// provisions `.venv` with protobuf, build and ty.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// Third Party Imports

// Local Imports
use harmos_domain::config::SchemaSettings;
use harmos_domain::operations::{Codec, Contract, Contracts, Grpc, OperationPolicy, RestartPolicy, RetryPolicy};
use harmos_domain::schema::{
    Cardinality, Described,
    graph::{Field, Kind, Schema},
};
use harmos_domain::{Aggregate, Identifier};
use harmos_packaging::{
    proto::Artifacts,
    python::{self, Settings},
};
use harmos_sdk::grpc::{self, DecodeInput};

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: std::time::Duration::from_secs(1),
    attempt_timeout: std::time::Duration::from_secs(1),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

// The Python Consumer Each Story Runs One Scene Of
const CONSUMER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/acceptance/bindings.py");

// The Generated Python Surface Of A Request Carrying Aggregates
const AGGREGATE_SNAPSHOT: &str = include_str!("fixtures/aggregate-bindings.pyi.snapshot");

// A Wide Schema Shared With The Domain's Graph Stories
const CHECKPOINT: &str = include_str!("../../../harmos-domain/tests/fixtures/checkpoint-schema.json");

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos_domain::message]
struct Payload {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message]
#[harmos_domain::operation(response = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Read"))]
struct Read {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message]
#[harmos_domain::operation(response = Payload, input = Payload, output = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Exchange"))]
struct Exchange {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message]
#[harmos_domain::operation(response = Payload, input = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Upload"))]
struct Upload {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message]
#[harmos_domain::operation(response = Payload, output = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Download"))]
struct Download {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message]
#[derive(PartialEq)]
struct Summary {
    aggregate: Aggregate,
    aggregates: Vec<Aggregate>,
}

// Two Producers Exported From One Catalog, Each In Its Own Directory
struct Exported {
    target: tempfile::TempDir,
    alpha: PathBuf,
    beta: PathBuf,
}

/* === Enums === */
#[harmos_domain::catalog]
enum Routes {
    Read(Read),
    Exchange(Exchange),
    Upload(Upload),
    Download(Download),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
// Four Streaming Shapes Plus A Wide, A Recursive And A Reserved-Name Schema
fn contracts() -> Vec<Contract> {
    let mut contracts = Contracts::of::<Routes>().unwrap();
    let template = contracts[0].clone();
    let checkpoint: Schema = serde_json::from_str(CHECKPOINT).unwrap();
    let recursive = Schema {
        root: 0,
        definitions: vec![vec![Field {
            name: Cow::Borrowed("next"),
            tag: 1,
            cardinality: Cardinality::Optional,
            kind: Kind::Message { definition: 0 },
        }]],
    };
    contracts.push(contract(&template, "fixture.checkpoint", "Checkpoint", checkpoint));
    contracts.push(contract(&template, "fixture.recursive", "Recursive", recursive));
    contracts.push(contract(&template, "fixture.reserved", "Reserved", reserved()));

    contracts
}

fn export() -> Exported {
    let contracts = contracts();
    let artifacts = Artifacts::new(&contracts).unwrap();
    let target = tempfile::tempdir().unwrap();
    let alpha = target.path().join("alpha");
    let beta = target.path().join("beta");
    python::generate(
        python::Catalog { contracts: &artifacts.operation_contracts().unwrap(), settings: &settings("bindings_alpha"), protoc: &protoc() },
        &alpha,
    )
    .unwrap();
    python::generate(
        python::Catalog { contracts: &artifacts.operation_contracts().unwrap(), settings: &settings("bindings_beta"), protoc: &protoc() },
        &beta,
    )
    .unwrap();

    Exported { target, alpha, beta }
}

fn reserved() -> Schema {
    Schema {
        root: 0,
        definitions: vec![
            vec![Field { name: Cow::Borrowed("Clear"), tag: 7, cardinality: Cardinality::Variant, kind: Kind::Message { definition: 1 } }],
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

// Wrap A Direction Schema In The Request And Response Root Of A New Contract
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

    Contract::new(operation, 2, Codec::Postcard, schema).unwrap().with_grpc(Some(Grpc::new("test.v1", "Shapes", method))).unwrap()
}

fn protoc() -> PathBuf {
    protoc_bin_vendored::protoc_bin_path().unwrap()
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
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let interpreter = repository.join(if cfg!(windows) { ".venv/Scripts/python.exe" } else { ".venv/bin/python" });
    assert!(interpreter.is_file(), "{} is missing; run `uv sync --locked --project tools/python --group test`", interpreter.display());
    let mut command = Command::new(interpreter);
    command
        .arg(CONSUMER)
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
#[test]
fn export_refuses_invalid() {
    let contracts = Contracts::of::<Routes>().unwrap();
    let artifacts = Artifacts::new(&contracts).unwrap();
    let target = tempfile::tempdir().unwrap();
    let output = target.path().join("package");
    let refused = |settings: &Settings| {
        python::generate(
            python::Catalog { contracts: &artifacts.operation_contracts().unwrap(), settings: settings, protoc: &protoc() },
            &output,
        )
    };

    // Selections, Module Names And Versions Python Cannot Carry
    let mut settings = settings("bindings_alpha");
    settings.operations = vec!["missing.operation".into()];
    assert!(matches!(refused(&settings), Err(python::Error::Selection(_))));
    settings.operations = vec![contracts[0].operation().id.to_string(); 2];
    assert!(matches!(refused(&settings), Err(python::Error::Collision(_))));
    settings.operations.clear();
    settings.module = "class".into();
    assert!(matches!(refused(&settings), Err(python::Error::Settings(_))));
    settings.module = "bindings_alpha".into();
    settings.version = "1.0.0-preview.1".parse().unwrap();
    assert!(matches!(refused(&settings), Err(python::Error::Settings(_))));
    settings.version = "1.0.0".parse().unwrap();

    // Contracts Without gRPC Routes Or Whose Escaped Names Collide
    let unsupported = contracts[0].clone().with_grpc(None).unwrap();
    let unsupported = Artifacts::new([&unsupported]).unwrap();
    assert!(matches!(
        python::generate(
            python::Catalog { contracts: &unsupported.operation_contracts().unwrap(), settings: &settings, protoc: &protoc() },
            &output
        ),
        Err(python::Error::Unsupported(_))
    ));
    let mut collision = reserved();
    collision.definitions[0].push(Field {
        name: Cow::Borrowed("Clear_"),
        tag: 8,
        cardinality: Cardinality::Variant,
        kind: Kind::Message { definition: 1 },
    });
    let collision = contract(&contracts[0], "fixture.collision", "Collision", collision);
    assert!(Artifacts::new([&collision]).is_err());
    assert!(!output.exists());

    // An Existing Directory Is Never Overwritten
    fs::create_dir(&output).unwrap();
    fs::write(output.join("owned.txt"), "preserved").unwrap();
    assert!(matches!(refused(&settings), Err(python::Error::Exists(_))));
    assert_eq!(fs::read_to_string(output.join("owned.txt")).unwrap(), "preserved");
}

#[test]
fn export_is_deterministic() {
    let artifacts = Artifacts::new(&contracts()).unwrap();
    let target = tempfile::tempdir().unwrap();
    let first = target.path().join("first");
    let repeat = target.path().join("repeat");
    python::generate(
        python::Catalog { contracts: &artifacts.operation_contracts().unwrap(), settings: &settings("bindings_alpha"), protoc: &protoc() },
        &first,
    )
    .unwrap();
    python::generate(
        python::Catalog { contracts: &artifacts.operation_contracts().unwrap(), settings: &settings("bindings_alpha"), protoc: &protoc() },
        &repeat,
    )
    .unwrap();

    assert_eq!(files(&first), files(&repeat));
}

#[test]
fn python_reads_rust() {
    // Rust Encodes, Python Decodes And Re-Encodes, Rust Decodes The Same Value
    let exported = export();
    let value = Payload { text: "line\n∞".into(), optional: Some(u32::MAX), values: vec![i64::MIN, -1, 0, i64::MAX] };
    let encoded = {
        let mut bytes = Vec::new();
        grpc::encode(&value, &mut bytes).unwrap();
        bytes
    };

    for echoed in both("wire", &exported, &encoded) {
        let decoded = grpc::decode::<Payload>(DecodeInput::new(&echoed[..])).unwrap();
        assert_eq!(decoded.text, value.text);
        assert_eq!(decoded.optional, value.optional);
        assert_eq!(decoded.values, value.values);
    }
}

#[test]
fn descriptors_mirror_contracts() {
    let exported = export();

    both("descriptors", &exported, &[]);
}

#[test]
fn producers_stay_isolated() {
    let exported = export();
    let arguments = [exported.target.path(), &exported.alpha, &exported.beta];

    scene("isolation", &arguments, false, &[]);
}

#[test]
fn stubs_type_check() {
    let exported = export();
    let arguments = [exported.target.path(), &exported.alpha, &exported.beta];

    scene("typing", &arguments, false, &[]);
}

#[test]
fn packages_install_offline() {
    let exported = export();
    let arguments = [exported.target.path(), &exported.alpha, &exported.beta];

    scene("install", &arguments, false, &[]);
}

#[test]
fn dynamic_projects_contracts() {
    let exported = export();

    both("dynamic", &exported, &[]);
}

#[test]
fn dynamic_refuses_graphs() {
    let exported = export();
    let arguments = [exported.target.path(), &exported.alpha, &exported.beta];

    scene("refusals", &arguments, false, &[]);
}

#[test]
fn aggregate_members_typed() {
    // The Generated Surface Of A Request Carrying Aggregates Is Stable
    let template = Contracts::of::<Routes>().unwrap()[0].clone();
    let schema = Schema::from_declaration(Summary::DEFINITION, &SchemaSettings::default()).unwrap();
    let summary = contract(&template, "fixture.aggregate", "Aggregate", schema);
    let artifacts = Artifacts::new([&summary]).unwrap();
    let target = tempfile::tempdir().unwrap();
    let package = target.path().join("package");
    python::generate(
        python::Catalog {
            contracts: &artifacts.operation_contracts().unwrap(),
            settings: &settings("bindings_aggregate"),
            protoc: &protoc(),
        },
        &package,
    )
    .unwrap();
    let module = package.join("src/bindings_aggregate");
    let messages = module.join("_wire/test/v1/ShapesMessages/Aggregate/messages_pb2.pyi");
    let generated = format!("{}\n{}", fs::read_to_string(module.join("__init__.pyi")).unwrap(), fs::read_to_string(messages).unwrap());
    assert_eq!(generated, AGGREGATE_SNAPSHOT, "{generated}");

    // Python Selects The Percentile Member And Its Rank As Typed Fields
    let value = Summary { aggregate: Aggregate::Percentile(95), aggregates: vec![Aggregate::Count, Aggregate::Percentile(99)] };
    let source = package.join("src");
    let output = scene("aggregate", &[&source], false, &{
        let mut bytes = Vec::new();
        grpc::encode(&value, &mut bytes).unwrap();
        bytes
    });
    assert!(grpc::decode::<Summary>(DecodeInput::new(&output[..])).unwrap() == value);
}
