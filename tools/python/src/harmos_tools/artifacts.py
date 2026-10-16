# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Authoring And Metadata Fixture Preparation"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

PREPARATION_TIMEOUT = 300
# Cold Cargo Builds Compile SDK Dependencies Before Tests Begin
BUILD_TIMEOUT = 900
VERSIONS = ("7.8.9", "7.8.10-beta.2+build")
BUILDS = (
    (False, "dev"),
    (True, "dev"),
    (False, "stripped-fixture"),
    (False, "plugin-fixture"),
    (True, "stripped-fixture"),
)
CATALOG_CASES = (
    "valid",
    "count",
    "bytes",
    "memory",
    "spin",
    "namespace",
    "schema",
    "duplicate",
)
GUESTS = (
    ("test-component", "test-component", None),
    ("route-component", "test-component", None),
    ("legacy-component", "componentize,kit", None),
    ("plugin-component", "componentize,kit", None),
    ("operation-component", "componentize,kit,streaming", None),
    ("operation-component", "componentize,kit,grpc,streaming", None),
    *(("catalog-component", "componentize,kit", case) for case in CATALOG_CASES),
)

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    prepare(parser.parse_args().output)


def prepare(root: Path) -> None:
    root = root.resolve()
    repository = Path(__file__).resolve().parents[4]
    root.mkdir(parents=True, exist_ok=True)
    manifest_path = root / "consumer.json"
    manifest_path.unlink(missing_ok=True)
    artifacts: dict[str, str] = {}
    libraries: dict[str, str] = {}
    target = root / "build"

    # Native Fixtures And Exact Compiler Artifacts Share One Build
    compiler = subprocess.run(
        [os.environ.get("RUSTC", "rustc"), "-vV"],
        check=True,
        capture_output=True,
        text=True,
        timeout=PREPARATION_TIMEOUT,
    )
    host = next(
        line.removeprefix("host: ")
        for line in compiler.stdout.splitlines()
        if line.startswith("host: ")
    )
    result = _run(
        repository,
        [
            "build",
            "--locked",
            "-p",
            "harmos",
            "-p",
            "harmos-sidecar",
            "-p",
            "harmos-guest",
            "--all-features",
            "--bins",
            "--lib",
            "--target",
            host,
            "--target-dir",
            str(target),
            "--message-format=json",
        ],
        capture=True,
    )
    messages = [
        json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")
    ]
    for message in messages:
        if message.get("reason") != "compiler-artifact":
            continue
        name = message["target"]["name"]
        executable = message.get("executable")
        if executable:
            destination = root / name / Path(executable).name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(executable, destination)
            artifacts[name] = str(destination)
        if name not in ("harmos", "harmos_sidecar", "serde", "harmos_macros"):
            continue
        for filename in message["filenames"]:
            path = Path(filename)
            extension = (
                (
                    ".dll"
                    if os.name == "nt"
                    else ".dylib"
                    if sys.platform == "darwin"
                    else ".so"
                )
                if name == "harmos_macros"
                else ".rlib"
            )
            native_dependencies = Path(host) / "debug/deps"
            if path.suffix != extension or (
                name == "serde"
                and not path.parent.parts[-3:] == native_dependencies.parts
            ):
                continue
            if name in libraries:
                raise RuntimeError(f"ambiguous compiler artifact for {name}")
            libraries[name] = str(path)
    if set(libraries) != {"harmos", "harmos_sidecar", "serde", "harmos_macros"}:
        raise RuntimeError(f"missing compiler artifacts: {libraries}")

    # Retain The Dependency Search Paths Used By Direct Compiler Fixtures
    dependencies = {}
    for kind, library in (("target", "serde"), ("host", "harmos_macros")):
        source_directory = Path(libraries[library]).parent
        destination = root / "dependencies" / kind
        if destination.exists():
            shutil.rmtree(destination)
        destination.mkdir(parents=True, exist_ok=True)
        for message in messages:
            if message.get("reason") != "compiler-artifact":
                continue
            for filename in message["filenames"]:
                source = Path(filename)
                if source.parent == source_directory and source.suffix in {
                    ".rlib",
                    ".rmeta",
                    ".so",
                    ".dll",
                    ".dylib",
                    ".a",
                    ".lib",
                }:
                    shutil.copy2(source, destination / source.name)
        dependencies[kind] = str(destination)
    for name, filename in list(libraries.items()):
        destination = root / f"compiler-{name}" / Path(filename).name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(filename, destination)
        libraries[name] = str(destination)

    artifacts.update(_tools(repository, root, target, host))

    # Keep Each Feature And Catalog Variant Before The Next Cargo Invocation
    for example, features, case in GUESTS:
        key = f"{example}:{features}:{case or 'default'}"
        environment = os.environ.copy()
        environment.pop("HARMOS_CATALOG_CASE", None)
        if case:
            environment["HARMOS_CATALOG_CASE"] = case
        _run(
            repository,
            [
                "build",
                "--locked",
                "-p",
                "harmos-guest",
                "--example",
                example,
                "--features",
                features,
                "--target",
                "wasm32-unknown-unknown",
                "--target-dir",
                str(target),
            ],
            env=environment,
        )
        source = (
            target
            / "wasm32-unknown-unknown/debug/examples"
            / (example.replace("-", "_") + ".wasm")
        )
        name = (
            example
            + ("-grpc" if "grpc" in features else "")
            + (f"-{case}" if case else "")
        )
        destination = root / name / "module.wasm"
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        component = destination.with_name("component.wasm")
        subprocess.run(
            [artifacts["harmos-component-fixture"], str(destination), str(component)],
            cwd=repository,
            check=True,
            timeout=PREPARATION_TIMEOUT,
        )
        artifacts[f"core:{key}"] = str(destination)
        artifacts[key] = str(component)

    # Build Both Versions In The Same Root To Exercise Incremental Metadata Refresh
    fixture = repository / "crates/harmos-macros/tests/fixtures/metadata"
    metadata = root / "metadata"
    source_root = metadata / "source"
    (source_root / "src").mkdir(parents=True, exist_ok=True)
    for name in ("main.rs", "lib.rs", "streams.rs", "operations.rs"):
        shutil.copyfile(fixture / name, source_root / "src" / name)
    shutil.copy2(repository / "Cargo.lock", source_root / "Cargo.lock")
    for version in VERSIONS:
        template = (fixture / "manifest.toml").read_text(encoding="utf-8")
        template = template.replace('"@VERSION@"', json.dumps(version))
        manifest = template.replace('"@SDK_PACKAGE@"', '"harmos-sidecar"').replace(
            '"@SDK_PATH@"', json.dumps(str(repository / "crates/harmos-sidecar"))
        )
        (source_root / "Cargo.toml").write_text(manifest, encoding="utf-8")
        if version == VERSIONS[0]:
            # Reset Prior Runs Before The Two-Version Incremental Scenario
            for guest, profile in dict.fromkeys(BUILDS):
                clean = [
                    "clean",
                    "--target-dir",
                    str(target),
                    "--package",
                    "cargo-metadata-fixture",
                    "--profile",
                    profile,
                ]
                if guest:
                    clean.extend(["--target", "wasm32-unknown-unknown"])
                _run(source_root, clean)
        for guest, profile in BUILDS:
            sdk = "harmos-guest" if guest else "harmos-sidecar"
            manifest = template.replace('"@SDK_PACKAGE@"', json.dumps(sdk)).replace(
                '"@SDK_PATH@"', json.dumps(str(repository / "crates" / sdk))
            )
            (source_root / "Cargo.toml").write_text(manifest, encoding="utf-8")
            command = [
                "build",
                "--offline",
                "--target-dir",
                str(target),
                "--profile",
                profile,
            ]
            directory = "debug" if profile == "dev" else profile
            if guest:
                command.extend(
                    [
                        "--lib",
                        "--no-default-features",
                        "--features",
                        "guest",
                        "--target",
                        "wasm32-unknown-unknown",
                    ]
                )
                source = (
                    target
                    / "wasm32-unknown-unknown"
                    / directory
                    / "cargo_metadata_fixture.wasm"
                )
            else:
                command.extend(["--bin", "cargo-metadata-fixture"])
                source = (
                    target
                    / directory
                    / ("cargo-metadata-fixture" + (".exe" if os.name == "nt" else ""))
                )
            # Restore The Selected Version's Output Even When Cargo Reuses Its Build Cache
            source.unlink(missing_ok=True)
            result = _run(
                source_root, [*command, "--message-format=json"], capture=True
            )
            reports = [
                json.loads(line)
                for line in result.stdout.splitlines()
                if line.startswith("{")
            ]
            outputs = [
                message
                for message in reports
                if message.get("reason") == "compiler-artifact"
                and Path(message["manifest_path"]) == source_root / "Cargo.toml"
                and message["target"]["name"]
                == ("cargo_metadata_fixture" if guest else "cargo-metadata-fixture")
            ]
            if len(outputs) != 1:
                raise RuntimeError(f"ambiguous metadata fixture artifacts: {outputs}")
            source = Path(
                outputs[0]["filenames"][0] if guest else outputs[0]["executable"]
            )
            destination = (
                metadata
                / version
                / ("guest" if guest else "native")
                / directory
                / source.name
            )
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)
            artifacts[f"metadata:{version}:{str(guest).lower()}:{directory}"] = str(
                destination
            )

    _catalogs(repository, root, target, artifacts)

    manifest = {
        "artifacts": {
            name: str(Path(path).relative_to(root)) for name, path in artifacts.items()
        },
        "libraries": {
            name: str(Path(path).relative_to(root)) for name, path in libraries.items()
        },
        "dependencies": {
            name: str(Path(path).relative_to(root))
            for name, path in dependencies.items()
        },
    }
    temporary = manifest_path.with_suffix(".tmp")
    temporary.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    temporary.replace(manifest_path)


def artifacts() -> dict[str, Path]:
    repository = Path(__file__).resolve().parents[4]
    manifest = Path(
        os.environ.get(
            "HARMOS_FIXTURES_MANIFEST",
            str(repository / "target/fixtures/harmos/consumer.json"),
        )
    )
    if not manifest.is_file():
        raise FileNotFoundError(
            f"prepared fixture manifest {manifest} is missing; run mise run test:integration:rust"
        )
    values = json.loads(manifest.read_text(encoding="utf-8"))["artifacts"]
    paths = {name: manifest.parent / path for name, path in values.items()}
    for name, path in paths.items():
        if not path.is_file():
            raise FileNotFoundError(
                f"prepared artifact {name} at {path} is missing; run mise run test:integration:rust"
            )
    return paths


# === Private Functions === #


def _tools(repository: Path, root: Path, target: Path, host: str) -> dict[str, str]:
    # Preserve Each Tool Feature Selection And Its Resolved Runtime Dependencies
    artifacts: dict[str, str] = {}
    tool_dependencies: dict[str, list[str]] = {}
    for variant, features in (
        ("none", ""),
        ("sidecar", "sidecar"),
        ("guest", "guest"),
        ("both", "sidecar,guest"),
    ):
        selection = ["-p", "cargo-harmos", "--no-default-features"]
        if features:
            selection.extend(["--features", features])
        built = _run(
            repository,
            [
                "build",
                "--locked",
                *selection,
                "--target",
                host,
                "--target-dir",
                str(target),
                "--message-format=json",
            ],
            capture=True,
        )
        executables = [
            message["executable"]
            for line in built.stdout.splitlines()
            if line.startswith("{")
            and (message := json.loads(line)).get("reason") == "compiler-artifact"
            and message["target"]["name"] == "cargo-harmos"
            and message.get("executable")
        ]
        if len(executables) != 1:
            raise RuntimeError(f"ambiguous cargo-harmos {variant} compiler artifact")
        source = Path(executables[0])
        destination = root / f"cargo-harmos-{variant}" / source.name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        artifacts[f"cargo-harmos-{variant}"] = str(destination)
        resolved = _run(
            repository,
            [
                "tree",
                "--locked",
                *selection,
                "--target",
                host,
                "--edges",
                "normal",
                "--prefix",
                "none",
                "--format",
                "{p}",
            ],
            capture=True,
        )
        tool_dependencies[variant] = sorted(
            {line.split()[0] for line in resolved.stdout.splitlines() if line.strip()}
        )
    dependency_record = root / "cargo-harmos-dependencies.json"
    dependency_record.write_text(
        json.dumps(tool_dependencies, indent=2) + "\n", encoding="utf-8"
    )
    artifacts["cargo-harmos-dependencies"] = str(dependency_record)
    return artifacts


def _catalogs(
    repository: Path, root: Path, target: Path, artifacts: dict[str, str]
) -> None:
    directory = root / "catalog-facades"
    facade = directory / "dokime"
    (facade / "src").mkdir(parents=True, exist_ok=True)
    (facade / "Cargo.toml").write_text(
        '[package]\nname = "dokime"\nversion = "0.0.0"\nedition = "2024"\n'
        "[dependencies]\nharmos = { path = "
        + json.dumps(str(repository / "crates/harmos"))
        + ", default-features = false }\n[workspace]\n",
        encoding="utf-8",
    )
    (facade / "src/lib.rs").write_text(
        "pub use harmos::catalog; pub use harmos as __harmos;",
        encoding="utf-8",
    )
    for package, dependency in (
        ("harmos", "harmos"),
        ("harmos", "sdk"),
        ("dokime", "dokime"),
        ("dokime", "sdk"),
    ):
        consumer = directory / f"{package}-{dependency}"
        (consumer / "src").mkdir(parents=True, exist_ok=True)
        path = facade if package == "dokime" else repository / "crates/harmos"
        (consumer / "Cargo.toml").write_text(
            '[package]\nname = "catalog-consumer"\nversion = "0.0.0"\nedition = "2024"\n[dependencies]\n'
            + f'{dependency} = {{ package = "{package}", path = {json.dumps(str(path))}, default-features = false }}\n[workspace]\n',
            encoding="utf-8",
        )
        shutil.copy2(repository / "Cargo.lock", consumer / "Cargo.lock")
        source = (
            repository / "crates/harmos-macros/tests/fixtures/catalog-consumer.rs"
        ).read_text(encoding="utf-8")
        (consumer / "src/lib.rs").write_text(
            source.replace("dependency::", f"{dependency}::"), encoding="utf-8"
        )
        result = _run(
            consumer, ["check", "--offline", "--target-dir", str(target)], capture=True
        )
        receipt = consumer / "result.json"
        receipt.write_text(
            json.dumps({"success": result.returncode == 0, "stdout": result.stdout}),
            encoding="utf-8",
        )
        artifacts[f"catalog:{package}:{dependency}"] = str(receipt)


def _run(
    root: Path,
    arguments: list[str],
    *,
    capture: bool = False,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    timeout = BUILD_TIMEOUT if arguments[0] == "build" else PREPARATION_TIMEOUT
    print(f"Preparing fixture ({timeout}s budget): " + " ".join(arguments), flush=True)
    return subprocess.run(
        [os.environ.get("CARGO", "cargo"), *arguments],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE if capture else None,
        text=True,
        timeout=timeout,
        env=env,
    )


# === Entry Point === #

if __name__ == "__main__":
    main()
