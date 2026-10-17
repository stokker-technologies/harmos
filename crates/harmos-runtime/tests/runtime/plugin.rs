/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use serde_json::json;
// Local Imports
use harmos_packaging::bundle::{Bundle, Executable, Facility, Identity, Limits, Source};
use harmos_domain::Registrar;
use harmos_domain::inventory::{Declaration, Facility as Inspected};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn identity() -> Identity {
    Identity {
        name: "cargo_plugin".into(),
        version: "2.3.4".parse().unwrap(),
        description: "Cargo description".into(),
        package: harmos_packaging::bundle::PackageMetadata {
            authors: vec!["Author <author@example.test>".into()],
            license: Some("Apache-2.0".into()),
            repository: Some("https://example.test/source".into()),
            homepage: Some("https://example.test".into()),
            documentation: Some("https://example.test/docs".into()),
            keywords: vec!["measurement".into()],
            categories: vec!["science".into()],
        },
    }
}

fn executable(facility: Facility) -> Executable {
    let mut registrar = Registrar::new();
    registrar
        .register("zeta.catalog", 1, &json!({"suite":"probe"}))
        .unwrap();
    registrar
        .register("alpha.catalog", 2, &json!([1, 2]))
        .unwrap();
    Executable {
        facility,
        target: "test-target".into(),
        registrations: registrar.finish(),
    }
}

fn fragments(facility: &str, owner: &str) -> Vec<u8> {
    use harmos_runtime::operations::{
        OperationPolicy, RestartPolicy, RetryPolicy,
        lifecycle::{Action, Descriptor},
    };
    let policy = OperationPolicy {
        timeout: std::time::Duration::from_secs(2),
        attempt_timeout: std::time::Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    let lifecycle = [
        Descriptor::new(Action::Initialize, policy).unwrap(),
        Descriptor::new(Action::Shutdown, policy).unwrap(),
    ];
    let mut fragments = vec![
        json!({"kind":"plugin","facility":facility,"id":owner,"version":"1.0.0","description":"Byte-only fixture","config":"Settings"}),
        json!({"kind":"contracts","facility":facility,"owner":owner,"provides":[],"requires":[],"lifecycle":lifecycle}),
        json!({"kind":"publications","facility":facility,"owner":owner,"streams":[]}),
    ];
    if facility == "sidecar" {
        fragments
            .push(json!({"kind":"observers","facility":facility,"owner":owner,"observers":[]}));
        fragments.push(json!({"kind":"restart","facility":facility,"owner":owner,"policy":{"credit":1,"sustained_health_ms":1000,"backoff_base_ms":1,"backoff_cap_ms":2}}));
    }
    let mut bytes = Vec::new();
    for fragment in fragments {
        bytes.extend_from_slice(harmos_domain::PREFIX);
        bytes.extend(serde_json::to_vec(&fragment).unwrap());
        bytes.push(0);
    }
    bytes
}

fn guest() -> Vec<u8> {
    // Valid empty component with one custom section carrying inert metadata.
    let mut bytes = b"\0asm\x0d\0\x01\0".to_vec();
    let mut section = vec![0]; // Empty custom-section name.
    section.extend(fragments("guest", "guest.fixture"));
    bytes.push(0);
    let mut len = section.len();
    loop {
        let byte = (len & 127) as u8;
        len >>= 7;
        bytes.push(byte | if len == 0 { 0 } else { 128 });
        if len == 0 {
            break;
        }
    }
    bytes.extend(section);
    bytes
}

fn asset() -> Bundle {
    Bundle::build(
        identity(),
        &[Source {
            path: "files/data",
            bytes: b"abc",
            executable: None,
        }],
        Limits::default(),
    )
    .unwrap()
}

fn archive(manifest: &[u8], entries: &[(&str, &[u8], tar::EntryType)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, bytes, kind) in std::iter::once(("manifest.json", manifest, tar::EntryType::Regular))
        .chain(entries.iter().copied())
    {
        let mut header = tar::Header::new_ustar();
        header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
        header.set_entry_type(kind);
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_cksum();
        builder.append(&header, bytes).unwrap();
    }
    builder.into_inner().unwrap()
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("independent_files_roundtrip", || {
            let staging = tempfile::tempdir().unwrap();
            let marker = staging.path().join("executed");
            let mut native = format!(
                "#!/bin/sh\nprintf executed > '{}'\nexit 73\n",
                marker.display()
            )
            .into_bytes();
            native.extend(fragments("sidecar", "native.fixture"));
            let guest = guest();
            let sources = [
                Source {
                    path: "files/icon.svg",
                    bytes: b"<svg/>",
                    executable: None,
                },
                Source {
                    path: "bin/probe",
                    bytes: &native,
                    executable: Some(executable(Facility::Sidecar)),
                },
                Source {
                    path: "guests/tool.wasm",
                    bytes: &guest,
                    executable: Some(executable(Facility::Guest)),
                },
            ];
            let bundle = Bundle::build(identity(), &sources, Limits::default()).unwrap();
            let reversed = [
                Source {
                    path: sources[2].path,
                    bytes: sources[2].bytes,
                    executable: sources[2].executable.clone(),
                },
                Source {
                    path: sources[1].path,
                    bytes: sources[1].bytes,
                    executable: sources[1].executable.clone(),
                },
                Source {
                    path: sources[0].path,
                    bytes: sources[0].bytes,
                    executable: None,
                },
            ];
            assert_eq!(
                bundle.bytes(),
                Bundle::build(identity(), &reversed, Limits::default())
                    .unwrap()
                    .bytes()
            );
            let inspected = Bundle::inspect(bundle.bytes(), Limits::default()).unwrap();
            assert_eq!(inspected.manifest().identity, identity());
            assert_eq!(
                inspected.declaration("bin/probe"),
                Some(&Declaration::inspect(&native, Inspected::Sidecar).unwrap())
            );
            assert_eq!(
                inspected.declaration("guests/tool.wasm"),
                Some(&Declaration::inspect(&guest, Inspected::Guest).unwrap())
            );
            let manifest = serde_json::to_value(inspected.manifest()).unwrap();
            assert!(
                manifest["files"][0]["executable"]
                    .get("declaration")
                    .is_none()
            );
            for source in &sources {
                assert_eq!(inspected.file(source.path), Some(source.bytes));
                let file = inspected
                    .manifest()
                    .files
                    .iter()
                    .find(|file| file.path == source.path)
                    .unwrap();
                assert_eq!(file.executable, source.executable);
            }
            let extracted = inspected.extract(staging.path()).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(extracted.path())
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777,
                    0o700
                );
                for (path, mode) in [
                    ("bin/probe", 0o755),
                    ("guests/tool.wasm", 0o644),
                    ("files/icon.svg", 0o644),
                ] {
                    assert_eq!(
                        std::fs::metadata(extracted.path().join(path))
                            .unwrap()
                            .permissions()
                            .mode()
                            & 0o777,
                        mode
                    );
                }
            }
            for source in &sources {
                assert_eq!(
                    std::fs::read(extracted.path().join(source.path)).unwrap(),
                    source.bytes
                );
            }
            assert_eq!(
                std::fs::read(extracted.path().join("manifest.json")).unwrap(),
                serde_json::to_vec(inspected.manifest()).unwrap()
            );
            assert!(
                !marker.exists(),
                "neither inspection nor extraction executes native code"
            );
            let path = extracted.path().to_owned();
            drop(extracted);
            assert!(!path.exists());
            assert!(inspected.extract(&staging.path().join("missing")).is_err());
        });
    }
    {
        scenario_runtime::check("executable_owned_registrations", || {
            let native = fragments("sidecar", "probe");
            let mut export = executable(Facility::Sidecar);
            let build = |export: Executable| {
                Bundle::build(
                    identity(),
                    &[Source {
                        path: "bin/probe",
                        bytes: &native,
                        executable: Some(export),
                    }],
                    Limits::default(),
                )
            };
            export.registrations.reverse();
            let valid = build(export.clone()).unwrap();
            assert_eq!(
                valid.manifest().files[0]
                    .executable
                    .as_ref()
                    .unwrap()
                    .registrations,
                executable(Facility::Sidecar).registrations
            );
            for change in 0..4 {
                let mut invalid = executable(Facility::Sidecar);
                match change {
                    0 => invalid.registrations[0].schema = 0,
                    1 => invalid.registrations[0].namespace = "INVALID".into(),
                    2 => invalid.registrations[0].payload = b"[] trailing".to_vec(),
                    _ => invalid.registrations.push(invalid.registrations[0].clone()),
                }
                assert!(build(invalid).is_err());
            }
            assert!(build(executable(Facility::Guest)).is_err());
            let sources = [
                Source {
                    path: "bin/one",
                    bytes: &native,
                    executable: Some(export.clone()),
                },
                Source {
                    path: "bin/two",
                    bytes: &native,
                    executable: Some(export.clone()),
                },
            ];
            assert!(Bundle::build(identity(), &sources, Limits::default()).is_err());
            let mut other_target = export.clone();
            other_target.target = "another-target".into();
            assert!(
                Bundle::build(
                    identity(),
                    &[
                        Source {
                            path: "bin/one",
                            bytes: &native,
                            executable: Some(export)
                        },
                        Source {
                            path: "bin/two",
                            bytes: &native,
                            executable: Some(other_target)
                        },
                    ],
                    Limits::default()
                )
                .is_ok()
            );
            assert!(
                Bundle::build(
                    identity(),
                    &[Source {
                        path: "bin/missing",
                        bytes: b"no declaration",
                        executable: Some(executable(Facility::Sidecar))
                    }],
                    Limits::default()
                )
                .is_err()
            );
        });
    }
    {
        scenario_runtime::check("archive_integrity", || {
            let bundle = asset();
            let manifest = serde_json::to_vec(bundle.manifest()).unwrap();
            let ordinary = [("files/data", b"abc".as_slice(), tar::EntryType::Regular)];
            assert_eq!(archive(&manifest, &ordinary), bundle.bytes());
            for entries in [
                vec![],
                vec![("files/other", b"abc".as_slice(), tar::EntryType::Regular)],
                vec![("files/data", b"bad".as_slice(), tar::EntryType::Regular)],
                vec![("files/data", b"long".as_slice(), tar::EntryType::Regular)],
                vec![ordinary[0], ordinary[0]],
                vec![
                    ordinary[0],
                    ("extra", b"".as_slice(), tar::EntryType::Regular),
                ],
            ] {
                assert!(Bundle::inspect(&archive(&manifest, &entries), Limits::default()).is_err());
            }
            for kind in [
                tar::EntryType::Symlink,
                tar::EntryType::Link,
                tar::EntryType::Directory,
                tar::EntryType::GNULongName,
                tar::EntryType::XGlobalHeader,
                tar::EntryType::XHeader,
                tar::EntryType::GNUSparse,
                tar::EntryType::Fifo,
            ] {
                assert!(
                    Bundle::inspect(
                        &archive(&manifest, &[("files/data", b"abc", kind)]),
                        Limits::default()
                    )
                    .is_err()
                );
            }
            for mutate in 0..5 {
                let mut bytes = bundle.bytes().to_vec();
                match mutate {
                    0 => bytes.push(0),
                    1 => bytes.extend_from_slice(bundle.bytes()),
                    2 => {
                        bytes.pop();
                    }
                    3 => bytes[512 + manifest.len()] = 1, // Nonzero TAR padding.
                    _ => bytes[0] ^= 1,                   // Broken header checksum.
                }
                assert!(Bundle::inspect(&bytes, Limits::default()).is_err());
            }
            for field in ["schema", "size", "sha256", "path", "unknown"] {
                let mut value = serde_json::to_value(bundle.manifest()).unwrap();
                match field {
                    "schema" => value["schema"] = json!(2),
                    "size" => value["files"][0]["size"] = json!(u64::MAX),
                    "sha256" => value["files"][0]["sha256"] = json!("0".repeat(64)),
                    "path" => value["files"][0]["path"] = json!("missing"),
                    _ => value["unknown"] = json!(true),
                }
                assert!(
                    Bundle::inspect(
                        &archive(&serde_json::to_vec(&value).unwrap(), &ordinary),
                        Limits::default()
                    )
                    .is_err()
                );
            }
            let duplicated = String::from_utf8(manifest).unwrap().replacen(
                "\"schema\":1",
                "\"schema\":1,\"schema\":1",
                1,
            );
            assert!(
                Bundle::inspect(
                    &archive(duplicated.as_bytes(), &ordinary),
                    Limits::default()
                )
                .is_err()
            );
            let mut impossible_size = bundle.bytes().to_vec();
            let mut header = tar::Header::from_byte_slice(&impossible_size[..512]).clone();
            header.set_size(u64::MAX);
            header.set_cksum();
            impossible_size[..512].copy_from_slice(header.as_bytes());
            assert!(Bundle::inspect(&impossible_size, Limits::default()).is_err());
        });
    }
    {
        scenario_runtime::check("portable_paths", || {
            let bundle = asset();
            let manifest = serde_json::to_vec(bundle.manifest()).unwrap();
            for path in [
                "",
                "/absolute",
                "../parent",
                "a/../b",
                "./here",
                "a//b",
                "a\\b",
                "C:/data",
                "con.txt",
                "a/NUL",
                "trail.",
                "with space",
                "manifest.json",
                "MANIFEST.JSON/a",
                "unicode-é",
            ] {
                let sources = [Source {
                    path,
                    bytes: b"abc",
                    executable: None,
                }];
                assert!(
                    Bundle::build(identity(), &sources, Limits::default()).is_err(),
                    "{path}"
                );
                assert!(
                    Bundle::inspect(
                        &archive(&manifest, &[(path, b"abc", tar::EntryType::Regular)]),
                        Limits::default()
                    )
                    .is_err(),
                    "{path}"
                );
            }
            for paths in [["a", "a"], ["A", "a"], ["A", "a/b"], ["a/b", "a"]] {
                assert!(
                    Bundle::build(
                        identity(),
                        &paths.map(|path| Source {
                            path,
                            bytes: b"abc",
                            executable: None
                        }),
                        Limits::default()
                    )
                    .is_err()
                );
            }
        });
    }
    {
        scenario_runtime::check("bounded_inputs", || {
            let bundle = asset();
            for limits in [
                Limits {
                    archive_bytes: bundle.bytes().len() - 1,
                    ..Limits::default()
                },
                Limits {
                    manifest_bytes: 1,
                    ..Limits::default()
                },
                Limits {
                    file_bytes: 2,
                    ..Limits::default()
                },
                Limits {
                    total_file_bytes: 2,
                    ..Limits::default()
                },
                Limits {
                    files: 0,
                    ..Limits::default()
                },
            ] {
                assert!(Bundle::inspect(bundle.bytes(), limits).is_err());
                assert!(
                    Bundle::build(
                        identity(),
                        &[Source {
                            path: "files/data",
                            bytes: b"abc",
                            executable: None
                        }],
                        limits
                    )
                    .is_err()
                );
            }
            let native = fragments("sidecar", "probe");
            assert!(
                Bundle::build(
                    identity(),
                    &[Source {
                        path: "bin/probe",
                        bytes: &native,
                        executable: Some(executable(Facility::Sidecar))
                    }],
                    Limits {
                        registrations: 1,
                        ..Limits::default()
                    }
                )
                .is_err()
            );
            assert!(Bundle::inspect(&bundle.bytes()[..512], Limits::default()).is_err());
            let empty = Bundle::build(identity(), &[], Limits::default()).unwrap();
            assert!(Bundle::inspect(empty.bytes(), Limits::default()).is_ok());
        });
    }
    {
        scenario_runtime::check("metadata_budgets_precede", || {
            let bundle = asset();
            let mut manifest = serde_json::to_value(bundle.manifest()).unwrap();
            // A typed decode would fail on size. The collection budget must win first.
            manifest["files"][0]["size"] = json!("not an integer");
            let bytes = archive(&serde_json::to_vec(&manifest).unwrap(), &[]);
            let error = Bundle::inspect(
                &bytes,
                Limits {
                    files: 0,
                    ..Limits::default()
                },
            )
            .err()
            .unwrap();
            assert!(error.to_string().contains("file count limit"), "{error}");

            manifest["files"][0]["executable"] = json!({"registrations":["not a registration"]});
            let bytes = archive(&serde_json::to_vec(&manifest).unwrap(), &[]);
            let error = Bundle::inspect(
                &bytes,
                Limits {
                    registrations: 0,
                    ..Limits::default()
                },
            )
            .err()
            .unwrap();
            assert!(
                error.to_string().contains("registration count limit"),
                "{error}"
            );

            let manifest_bytes = serde_json::to_vec(bundle.manifest()).unwrap().len();
            let exact = Limits {
                manifest_bytes,
                archive_bytes: bundle.bytes().len(),
                file_bytes: 3,
                total_file_bytes: 3,
                files: 1,
                registrations: 0,
            };
            assert!(
                Bundle::build(
                    identity(),
                    &[Source {
                        path: "files/data",
                        bytes: b"abc",
                        executable: None
                    }],
                    exact
                )
                .is_ok()
            );
            assert!(Bundle::inspect(bundle.bytes(), exact).is_ok());

            let native = fragments("sidecar", "probe");
            let one = Bundle::build(
                identity(),
                &[Source {
                    path: "bin/one",
                    bytes: &native,
                    executable: Some(executable(Facility::Sidecar)),
                }],
                Limits::default(),
            )
            .unwrap();
            let limit = serde_json::to_vec(one.manifest()).unwrap().len();
            let mut other = executable(Facility::Sidecar);
            other.target = "other-target".into();
            let error = Bundle::build(
                identity(),
                &[
                    Source {
                        path: "bin/one",
                        bytes: &native,
                        executable: Some(executable(Facility::Sidecar)),
                    },
                    Source {
                        path: "bin/two",
                        bytes: &native,
                        executable: Some(other),
                    },
                ],
                Limits {
                    manifest_bytes: limit,
                    ..Limits::default()
                },
            )
            .err()
            .unwrap();
            assert!(error.to_string().contains("manifest byte limit"), "{error}");
        });
    }
}
