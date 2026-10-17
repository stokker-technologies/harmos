/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Verify Generated Standard Messages, Producer Isolation And Installed Packages.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
// Third Party Imports
// Local Imports
use super::{Payload, Routes, scenario_runtime};
use harmos_domain::{
    Aggregate, Identifier, Protobuf,
    config::SchemaSettings,
    operations::{Codec, Contract, Contracts, Grpc},
    schema::{
        Cardinality, Described,
        graph::{Field, Kind, Schema},
    },
};

use harmos_packaging::{
    proto::Artifacts,
    python::{self, Settings},
};

/* === Constants === */
// Read A Rust Summary, Check Its Typed Members, Then Rebuild It From Python
const AGGREGATE_SCRIPT: &str = r#"
import sys

sys.path.insert(0, sys.argv[1])
import bindings_aggregate as bindings

received = bindings.FixtureAggregateRequest.FromString(sys.stdin.buffer.read())
assert received.aggregate.WhichOneof("_harmos_variant") == "Percentile"
assert received.aggregate.Percentile.rank == 95
assert [item.WhichOneof("_harmos_variant") for item in received.aggregates] == ["Count", "Percentile"]
built = bindings.FixtureAggregateRequest()
built.aggregate.Percentile.rank = 95
built.aggregates.add().Count.SetInParent()
built.aggregates.add().Percentile.rank = 99
sys.stdout.buffer.write(built.SerializeToString())
"#;

// The Generated Python Surface Of A Request Carrying Aggregates
const AGGREGATE_SNAPSHOT: &str = include_str!("../fixtures/aggregate-bindings.pyi");

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Summary {
    aggregate: Aggregate,
    aggregates: Vec<Aggregate>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn check() {
    scenario_runtime::check("python_bindings_preflight", || {
        let contracts = Contracts::of::<Routes>().unwrap();
        let artifacts = Artifacts::new(&contracts).unwrap();
        let target = tempfile::tempdir().unwrap();
        let output = target.path().join("package");
        let mut settings = settings("bindings_alpha");
        settings.operations = vec!["missing.operation".into()];
        assert!(matches!(
            python::generate(
                python::Catalog {
                    contracts: &artifacts.operation_contracts().unwrap(),
                    settings: &settings,
                    protoc: &protoc()
                },
                &output
            ),
            Err(python::Error::Selection(_))
        ));
        assert!(!output.exists());
        settings.operations = vec![contracts[0].operation().id.to_string(); 2];
        assert!(matches!(
            python::generate(
                python::Catalog {
                    contracts: &artifacts.operation_contracts().unwrap(),
                    settings: &settings,
                    protoc: &protoc()
                },
                &output
            ),
            Err(python::Error::Collision(_))
        ));
        settings.operations.clear();
        settings.module = "class".into();
        assert!(matches!(
            python::generate(
                python::Catalog {
                    contracts: &artifacts.operation_contracts().unwrap(),
                    settings: &settings,
                    protoc: &protoc()
                },
                &output
            ),
            Err(python::Error::Settings(_))
        ));
        settings.module = "bindings_alpha".into();
        settings.version = "1.0.0-preview.1".parse().unwrap();
        assert!(matches!(
            python::generate(
                python::Catalog {
                    contracts: &artifacts.operation_contracts().unwrap(),
                    settings: &settings,
                    protoc: &protoc()
                },
                &output
            ),
            Err(python::Error::Settings(_))
        ));
        settings.version = "1.0.0".parse().unwrap();
        let unsupported = contracts[0].clone().with_grpc(None).unwrap();
        let unsupported = Artifacts::new([&unsupported]).unwrap();
        assert!(matches!(
            python::generate(
                python::Catalog {
                    contracts: &unsupported.operation_contracts().unwrap(),
                    settings: &settings,
                    protoc: &protoc()
                },
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
        fs::create_dir(&output).unwrap();
        fs::write(output.join("owned.txt"), "preserved").unwrap();
        assert!(matches!(
            python::generate(
                python::Catalog {
                    contracts: &artifacts.operation_contracts().unwrap(),
                    settings: &settings,
                    protoc: &protoc()
                },
                &output
            ),
            Err(python::Error::Exists(_))
        ));
        assert_eq!(
            fs::read_to_string(output.join("owned.txt")).unwrap(),
            "preserved"
        );
    });
    scenario_runtime::check("python_bindings_install_and_wire", || {
        let mut contracts = Contracts::of::<Routes>().unwrap();
        let template = contracts[0].clone();
        let checkpoint: Schema =
            serde_json::from_str(include_str!("../fixtures/checkpoint-schema.json")).unwrap();
        contracts.push(contract(
            &template,
            "fixture.checkpoint",
            "Checkpoint",
            checkpoint,
        ));
        let recursive = Schema {
            root: 0,
            definitions: vec![vec![Field {
                name: Cow::Borrowed("next"),
                tag: 1,
                cardinality: Cardinality::Optional,
                kind: Kind::Message { definition: 0 },
            }]],
        };
        contracts.push(contract(
            &template,
            "fixture.recursive",
            "Recursive",
            recursive,
        ));
        contracts.push(contract(
            &template,
            "fixture.reserved",
            "Reserved",
            reserved(),
        ));
        let artifacts = Artifacts::new(&contracts).unwrap();
        let target = tempfile::tempdir().unwrap();
        let alpha = target.path().join("alpha");
        let beta = target.path().join("beta");
        let repeat = target.path().join("repeat");
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
        python::generate(
            python::Catalog {
                contracts: &artifacts.operation_contracts().unwrap(),
                settings: &settings("bindings_alpha"),
                protoc: &protoc(),
            },
            &repeat,
        )
        .unwrap();
        assert_eq!(files(&alpha), files(&repeat));
        let value = Payload {
            text: "line\n∞".into(),
            optional: Some(u32::MAX),
            values: vec![i64::MIN, -1, 0, i64::MAX],
        };
        fs::write(
            target.path().join("rust.bin"),
            Protobuf::encode_to_vec(&value),
        )
        .unwrap();
        let script = target.path().join("verify.py");
        fs::write(&script, include_str!("python.py")).unwrap();
        run(&script, target.path(), &alpha, &beta, false);
        let decoded =
            <Payload as Protobuf>::decode(&fs::read(target.path().join("python.bin")).unwrap())
                .unwrap();
        assert_eq!(decoded.text, value.text);
        assert_eq!(decoded.optional, value.optional);
        assert_eq!(decoded.values, value.values);
        run(&script, target.path(), &alpha, &beta, true);
    });
    scenario_runtime::check("python_bindings_aggregate_typed", || {
        let template = Contracts::of::<Routes>().unwrap()[0].clone();
        let schema =
            Schema::from_declaration(Summary::DEFINITION, &SchemaSettings::default()).unwrap();
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
        let generated = format!(
            "{}\n{}",
            fs::read_to_string(module.join("__init__.pyi")).unwrap(),
            fs::read_to_string(messages).unwrap()
        );
        assert_eq!(generated, AGGREGATE_SNAPSHOT, "{generated}");
        // Python Selects The Percentile Member And Its Rank As Typed Fields
        let value = Summary {
            aggregate: Aggregate::Percentile(95),
            aggregates: vec![Aggregate::Count, Aggregate::Percentile(99)],
        };
        let script = target.path().join("aggregate.py");
        fs::write(&script, AGGREGATE_SCRIPT).unwrap();
        let output = python(
            &script,
            &package.join("src"),
            &Protobuf::encode_to_vec(&value),
        );
        assert!(<Summary as Protobuf>::decode(&output).unwrap() == value);
    });
}

/* === Private Functions === */
fn reserved() -> Schema {
    Schema {
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

fn contract(template: &Contract, id: &str, method: &'static str, mut schema: Schema) -> Contract {
    let mut operation = template.operation().clone();
    operation.id = Identifier::new(id).unwrap();
    let direction = schema.root;
    schema.root = schema.definitions.len();
    schema.definitions.push(vec![
        Field {
            name: Cow::Borrowed("request"),
            tag: 1,
            cardinality: Cardinality::Required,
            kind: Kind::Message {
                definition: direction,
            },
        },
        Field {
            name: Cow::Borrowed("response"),
            tag: 2,
            cardinality: Cardinality::Required,
            kind: Kind::Message {
                definition: direction,
            },
        },
    ]);
    Contract::new(operation, 2, Codec::Postcard, schema)
        .unwrap()
        .with_grpc(Some(Grpc::new("test.v1", "Shapes", method)))
        .unwrap()
}

fn python(script: &Path, source: &Path, input: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let interpreter = repository.join(if cfg!(windows) {
        ".venv/Scripts/python.exe"
    } else {
        ".venv/bin/python"
    });
    let mut child = Command::new(interpreter)
        .arg(script)
        .arg(source)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

fn protoc() -> PathBuf {
    protoc_bin_vendored::protoc_bin_path().unwrap()
}

fn files(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut directories = vec![root.to_owned()];
    let mut result = std::collections::BTreeMap::new();
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                directories.push(entry.path());
            } else {
                result.insert(
                    entry.path().strip_prefix(root).unwrap().to_owned(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    result
}

fn run(script: &Path, target: &Path, alpha: &Path, beta: &Path, pure: bool) {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let python = repository.join(if cfg!(windows) {
        ".venv/Scripts/python.exe"
    } else {
        ".venv/bin/python"
    });
    let mut command = Command::new(python);
    command.env_remove("HARMOS_BINDINGS_SKIP_INSTALL");
    command.env_remove("PROTOCOL_BUFFERS_PYTHON_IMPLEMENTATION");
    command
        .arg(script)
        .arg(target)
        .arg(alpha)
        .arg(beta)
        .current_dir(target);
    if pure {
        command
            .env("PROTOCOL_BUFFERS_PYTHON_IMPLEMENTATION", "python")
            .env("HARMOS_BINDINGS_SKIP_INSTALL", "1");
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

/* === Tests === */
