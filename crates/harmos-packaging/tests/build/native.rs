/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use harmos_packaging::{Artifact, BuildCancellation, Error, ProcessSettings, cargo, native};
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

#[test]
fn native_collection_retains_selected_binaries_and_symbols() {
    let artifact = |name: &str, path: &str, test: bool| {
        json!({
            "reason":"compiler-artifact", "package_id":"plugin-id",
            "target":{"name":name,"kind":["bin"]}, "profile":{"test":test},
            "executable":path,"filenames":[path,format!("deps/{name}.pdb")]
        })
    };
    let messages = [
        artifact("driver", "driver.exe", false),
        artifact("helper", "helper.exe", false),
        artifact("driver", "test-driver.exe", true),
        json!({"reason":"build-finished","success":true}),
    ];
    let encoded = messages.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
    let selected = BTreeMap::from([("plugin-id".into(), "plugin".into())]);
    let outputs =
        cargo::collect(encoded.as_bytes(), &selected).unwrap().executables("plugin", &["driver".into(), "helper".into()]).unwrap();
    assert_eq!(
        outputs,
        BTreeMap::from([
            ("driver".into(), (PathBuf::from("driver.exe"), Some(PathBuf::from("deps/driver.pdb")))),
            ("helper".into(), (PathBuf::from("helper.exe"), Some(PathBuf::from("deps/helper.pdb")))),
        ])
    );
    assert!(cargo::collect(encoded.as_bytes(), &selected).unwrap().executables("plugin", &["absent".into()]).is_err());
}

#[test]
fn catalog_capture_bounds_output_and_settles_process_tree() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("sidecar.rs");
    let path = root.path().join(format!("sidecar{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(
        &source,
        r#"
        use std::{fs,thread,time::Duration};
        fn main() {
            if std::env::args().any(|arg| arg == "--child") {
                fs::write("ready", "ready").unwrap();
                thread::sleep(Duration::from_millis(1500));
                fs::write("late", "escaped").unwrap();
                return;
            }
            match fs::read_to_string("mode").unwrap_or_default().as_str() {
                "flood" => loop { println!("{}", "x".repeat(4096)); },
                "detach" => {
                    let _child = std::process::Command::new(std::env::current_exe().unwrap()).arg("--child")
                        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
                    while !std::path::Path::new("ready").exists() { thread::sleep(Duration::from_millis(10)); }
                    print!("[ ]\n");
                },
                "wait" => {
                    let _child = std::process::Command::new(std::env::current_exe().unwrap()).arg("--child").spawn().unwrap();
                    thread::sleep(Duration::from_secs(30));
                },
                _ => print!("[ ]\n"),
            }
        }
    "#,
    )
    .unwrap();
    assert!(std::process::Command::new("rustc").arg(&source).arg("-o").arg(&path).status().unwrap().success());
    let artifact = Artifact::read("plugin".into(), "sidecar".into(), path).unwrap();
    let settings = ProcessSettings {
        timeout: Some(Duration::from_secs(1)),
        stdout_bytes: 64,
        stderr_bytes: 4096,
    };
    let exported = native::catalog(&artifact, Some(root.path()), settings, &BuildCancellation::default()).unwrap();
    assert!(exported.registrations.is_empty());
    assert_eq!(exported.output_bytes, 4);
    std::fs::write(root.path().join("mode"), "flood").unwrap();
    let failure = native::catalog(&artifact, Some(root.path()), settings, &BuildCancellation::default()).unwrap_err();
    assert!(failure.to_string().contains("byte limit"), "{failure}");
    std::fs::write(root.path().join("mode"), "wait").unwrap();
    assert!(matches!(native::catalog(&artifact, Some(root.path()), settings, &BuildCancellation::default()), Err(Error::Timeout)));
    assert!(root.path().join("ready").exists(), "the descendant started before its deadline");
    std::thread::sleep(Duration::from_secs(1));
    assert!(!root.path().join("late").exists(), "the timed-out descendant did not survive capture");
    std::fs::remove_file(root.path().join("ready")).unwrap();
    std::fs::write(root.path().join("mode"), "detach").unwrap();
    native::catalog(&artifact, Some(root.path()), settings, &BuildCancellation::default()).unwrap();
    assert!(root.path().join("ready").exists());
    std::thread::sleep(Duration::from_millis(1700));
    assert!(!root.path().join("late").exists(), "successful capture retained its leader's descendants");
    std::fs::remove_file(root.path().join("ready")).unwrap();
    std::fs::write(root.path().join("mode"), "wait").unwrap();
    let cancellation = BuildCancellation::default();
    std::thread::scope(|scope| {
        let child =
            scope.spawn(|| native::catalog(&artifact, Some(root.path()), ProcessSettings { timeout: None, ..settings }, &cancellation));
        let started = std::time::Instant::now();
        while !root.path().join("ready").exists() && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let ready = root.path().join("ready").exists();
        cancellation.cancel();
        assert!(matches!(child.join().unwrap(), Err(Error::Cancelled)));
        assert!(ready, "the cancelled descendant started");
    });
}
