# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build The Workspace And The Schematic System Fixture Once, Then Stamp Them"""

# === Imports === #

# Standard Library Imports
import argparse
import hashlib
import importlib.resources
import json
import os
import shutil
import sys
from pathlib import Path

from ..test import archive

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[5]
TARGET = ROOT / "target"
FIXTURES = TARGET / "fixtures"
STAMP = FIXTURES / "stamp.json"
SCHEMATIC = ROOT / "examples/schematic/Cargo.toml"
GUESTS = {"schematic-guest": SCHEMATIC, "harmos-sandbox-guest": ROOT / "crates/harmos-guest/tests/fixtures/sandbox/Cargo.toml"}
LIBRARIES = ("harmos", "harmos_sidecar", "harmos_macros", "serde")

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release", "bench"), default="dev")
    profile = parser.parse_args().profile
    temporary = TARGET / "tmp"
    temporary.mkdir(parents=True, exist_ok=True)
    os.environ["TMPDIR"] = str(temporary)
    os.environ["CARGO_TARGET_DIR"] = str(TARGET)
    before = archive.tree(ROOT)
    receipt = archive.build(ROOT, profile, ["--all-features"])
    workspace = _messages(receipt)
    # The Archive Holds Test Targets Only; The Authoring Tool Binary Is Its Own Capture.
    tooling = archive.compile(ROOT, profile, ["-p", "cargo-harmos", "--all-features"])
    # The Facade Has No Test Dependents, So Its Library Is Its Own Capture For The Authoring Stories.
    facade = archive.compile(ROOT, profile, ["-p", "harmos", "--all-features", "--lib"])
    native = archive.compile(SCHEMATIC.parent, profile, ["-p", "schematic-cli", "-p", "schematic-sidecar"])
    schematic = _messages(native)
    producers = [receipt, tooling, facade, native]
    tool = _executable(_messages(tooling), "cargo-harmos")
    archive.verify_outputs(tooling, [tool])
    shutil.rmtree(FIXTURES, ignore_errors=True)
    FIXTURES.mkdir(parents=True)
    files = {}
    artifacts = {"cargo-harmos": _keep(tool, "cargo-harmos", [tooling], files)}
    builds = {}
    for package, manifest, backend, capture in [
        ("schematic-sidecar", SCHEMATIC, "sidecar", native),
        *((package, manifest, "guest", None) for package, manifest in GUESTS.items()),
    ]:
        if capture is None:
            capture = archive.compile(manifest.parent, profile, ["-p", package, "--target", "wasm32-unknown-unknown"])
            producers.append(capture)
        built, collected, product = _collect(tool, manifest, package, backend, profile, tooling, capture)
        producers.append(product)
        artifacts[package], builds[package] = _stage_build(built, collected, package, [capture, product], files)
    artifacts["schematic-cli"] = _keep(_executable(schematic, "schematic-cli"), "schematic-cli", [native], files)
    libraries = _libraries([*workspace, *_messages(facade)], [receipt, facade], files)
    exported = _offline_inputs(profile)
    producers.append(exported)
    for name in exported["exports"]:
        source = Path(name)
        _copy(source, FIXTURES / source.relative_to(TARGET / "dependencies"), [exported], files)
    if archive.tree(ROOT) != before:
        raise RuntimeError("sources changed while fixtures were building; rebuild the complete candidate")
    archive.stamp(
        ROOT,
        producers,
        files,
        {
            "artifacts": artifacts,
            "libraries": libraries,
            "builds": builds,
            "build_roots": {name: str(ROOT) for name in builds},
            "dependencies": "libraries",
            "vendor": "vendor",
            "wheels": "wheels",
            "requirements": "requirements.txt",
            "executables": [artifacts[name] for name in ("cargo-harmos", "schematic-cli", "schematic-sidecar")],
        },
    )


# === Private Functions === #


def _messages(record: dict) -> list[dict]:
    content = _resolve(record["messages"]["path"]).read_bytes()
    if hashlib.sha256(content).hexdigest() != record["messages"]["sha256"]:
        raise RuntimeError("compiler messages differ from their original producer receipt")
    return [json.loads(line) for line in content.decode("utf-8").splitlines() if line.startswith("{")]


def _resolve(path: str) -> Path:
    selected = Path(path)
    return selected if selected.is_absolute() else ROOT / selected


def _collect(tool: Path, manifest: Path, package: str, backend: str, profile: str, owner: dict, capture: dict) -> tuple[dict, Path, dict]:
    destination = TARGET / "collected" / package
    shutil.rmtree(destination, ignore_errors=True)
    result = destination / "build.json"
    outputs = [result]
    if backend == "guest":
        messages = _messages(capture)
        modules = {
            Path(filename)
            for message in messages
            if message.get("reason") == "compiler-artifact"
            and not message["profile"]["test"]
            and message["target"]["name"] == package.replace("-", "_")
            for filename in message["filenames"]
            if filename.endswith(".wasm")
        }
        if len(modules) != 1:
            raise RuntimeError(f"expected one raw {package} module, found {modules}")
        module = modules.pop()
        archive.verify_outputs(capture, [module])
        if not module.read_bytes().startswith(b"\0asm\x0d\0\x01\0"):
            transformed = module.with_suffix(".component.wasm")
            transformed.unlink(missing_ok=True)
            outputs.append(transformed)
    command = [
        str(tool),
        "harmos",
        backend,
        "build",
        "--manifest-path",
        str(manifest),
        "-p",
        package,
        "--profile",
        profile,
        "--target",
        capture["target"],
        "--from-cargo-json",
        str(_resolve(capture["messages"]["path"])),
        "--metadata-json",
        str(_resolve(capture["metadata"]["path"])),
        "--archive-json",
        str(_resolve(capture["receipt"])),
        "--json",
    ]
    produced = archive.native(
        ROOT,
        profile,
        "package",
        [command],
        outputs,
        locks=[ROOT / "Cargo.lock", manifest.parent / "Cargo.lock"],
        inputs=[tool, _resolve(capture["receipt"])],
        parents=[owner, capture],
        stdout={0: result},
    )
    return json.loads(result.read_text(encoding="utf-8")), result, produced


def _offline_inputs(profile: str) -> dict:
    """Capture Locked Native Dependency Exports Before Copying Into Fixtures."""
    destination = TARGET / "dependencies"
    shutil.rmtree(destination, ignore_errors=True)
    manifests = sorted({ROOT / "Cargo.toml", SCHEMATIC, *GUESTS.values()})
    vendor = ["cargo", "vendor", "--locked", "--versioned-dirs"]
    for manifest in manifests:
        if manifest != ROOT / "Cargo.toml":
            vendor += ["--sync", str(manifest)]
    vendor.append(str(destination / "vendor"))
    requirements = destination / "requirements.txt"
    projects = [ROOT / "tools/python", *(ROOT / f"crates/cargo-harmos/templates/{kind}/tools/python" for kind in ("sidecar", "guest"))]
    commands = [vendor]
    captured = {}
    requirements_files = []
    for index, project in enumerate(projects):
        selected = requirements if index == 0 else destination / f"starter-{project.parents[1].name}.txt"
        captured[len(commands)] = selected
        requirements_files.append(selected)
        commands.append(["uv", "export", "--locked", "--all-groups", "--no-emit-project", "--project", str(project)])
    bundled = importlib.resources.files("ensurepip").joinpath("_bundled")
    wheels = [path for path in bundled.iterdir() if path.name.startswith("pip-") and path.name.endswith(".whl")]
    if len(wheels) != 1:
        raise RuntimeError("the pinned Python must provide its bundled pip wheel to stage offline inputs")
    launch = "import runpy,sys; sys.path.insert(0,sys.argv.pop(1)); runpy.run_module('pip',run_name='__main__')"
    download = [sys.executable, "-c", launch, str(wheels[0]), "download", "--dest", str(destination / "wheels"), "--require-hashes"]
    for selected in requirements_files:
        download += ["--requirement", str(selected)]
    commands.append(download)
    locks = [manifest.parent / "Cargo.lock" for manifest in manifests] + [project / "uv.lock" for project in projects]
    inputs = [*manifests, *(project / "pyproject.toml" for project in projects), Path(str(wheels[0]))]
    if (ROOT / ".cargo").is_dir():
        inputs.append(ROOT / ".cargo")
    return archive.native(ROOT, profile, "dependency-export", commands, [destination], locks=locks, inputs=inputs, stdout=captured)


def _stage_build(build: dict, collected: Path, package: str, producers: list[dict], files: dict) -> tuple[str, str]:
    selected = [artifact for artifact in build["artifacts"] if artifact["producer"] == package]
    if len(selected) != 1:
        raise RuntimeError(f"{package} did not produce exactly one artifact: {selected}")
    artifact = selected[0]
    root = FIXTURES / "builds" / package
    source = Path(artifact["path"])
    destination = root / source.relative_to(ROOT)
    if _copy(source, destination, producers, files) != artifact["sha256"]:
        raise RuntimeError("typed artifact digest differs from its original producer")
    for schema in build["schemas"]:
        source = Path(schema["directory"])
        for name, expected in schema["files"].items():
            original = source / name
            if _copy(original, root / original.relative_to(ROOT), producers, files) != expected:
                raise RuntimeError("typed schema digest differs from its original producer")
    # Original Build JSON Remains Byte-Exact; Packaging Relocates Its Typed Paths.
    receipt = FIXTURES / "receipts" / f"{package}.json"
    _copy(collected, receipt, producers, files)
    return destination.relative_to(FIXTURES).as_posix(), receipt.relative_to(FIXTURES).as_posix()


def _copy(source: Path, destination: Path, producers: list[dict], files: dict) -> str:
    owner = next(
        (
            record
            for record in producers
            if str(source)
            in {
                **record.get("artifacts", {}),
                **record.get("generated", {}),
                **record.get("exports", {}),
            }
        ),
        None,
    )
    if owner is None:
        raise RuntimeError(f"staged input has no original producer: {source}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    expected = archive.verify_copy(owner, source, destination)
    files[destination.relative_to(ROOT).as_posix()] = expected
    return expected


def _executable(messages: list[dict], name: str) -> Path:
    found = {
        message["executable"]
        for message in messages
        if message.get("reason") == "compiler-artifact"
        and message["target"]["name"] == name
        and message.get("executable")
        and not message["profile"]["test"]
    }
    if len(found) != 1:
        raise RuntimeError(f"expected one {name} executable, found {sorted(found)}")
    return Path(found.pop())


def _libraries(messages: list[dict], producers: list[dict], files: dict) -> dict[str, str]:
    """Transfer exact compiler-emitted libraries needed by the authoring refusal stories.

    A library built twice (a build-dependency feature set beside the normal one) resolves to
    the build whose features include every other's.
    """
    found: dict[str, dict[str, set[str]]] = {name: {} for name in LIBRARIES}
    destination = FIXTURES / "libraries"
    destination.mkdir()
    for message in messages:
        if message.get("reason") != "compiler-artifact":
            continue
        name = message["target"]["name"]
        for filename in message["filenames"]:
            path = Path(filename)
            if path.suffix not in {".rlib", ".rmeta", ".so", ".dylib", ".dll"}:
                continue
            if not (destination / path.name).exists():
                _copy(path, destination / path.name, producers, files)
            if name not in found:
                continue
            suffix = (
                (".dll" if os.name == "nt" else ".dylib" if sys.platform == "darwin" else ".so") if name == "harmos_macros" else ".rlib"
            )
            if path.suffix == suffix:
                found[name][(destination / path.name).relative_to(FIXTURES).as_posix()] = set(message["features"])
    selected = {name: _widest(builds) for name, builds in found.items()}
    ambiguous = {name: sorted(found[name]) for name, path in selected.items() if path is None}
    if ambiguous:
        raise RuntimeError(f"compiler libraries are missing or ambiguous: {ambiguous}")
    return {name: path for name, path in selected.items() if path is not None}


def _widest(builds: dict[str, set[str]]) -> str | None:
    widest = [path for path, features in builds.items() if all(other <= features for other in builds.values())]
    return widest[0] if len(widest) == 1 else None


def _keep(source: Path, name: str, producers: list[dict], files: dict) -> str:
    destination = FIXTURES / name / source.name
    destination.parent.mkdir(parents=True, exist_ok=True)
    _copy(source, destination, producers, files)
    return destination.relative_to(FIXTURES).as_posix()


# === Entry Point === #

if __name__ == "__main__":
    main()
