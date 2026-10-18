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
import stat
import subprocess
import sys
from pathlib import Path

# Third Party Imports

# Local Imports
from ..test import archive

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[5]
TARGET = ROOT / "target"
FIXTURES = TARGET / "fixtures"
STAMP = FIXTURES / "stamp.json"
CAPTURES = TARGET / "captures"
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
    archive.build(ROOT, profile, ["--all-features"])
    # The Archive Holds Test Targets Only; The Authoring Tool Binary Is Its Own Capture, Beside
    # The Facade Library The Authoring Stories Link (Nothing Under Test Depends On It).
    tooling = _compile("tooling", ROOT, profile, ["-p", "cargo-harmos", "-p", "harmos", "--all-features"])
    native = _compile("schematic", SCHEMATIC.parent, profile, ["-p", "schematic-cli", "-p", "schematic-sidecar"])
    tool = _executable(_messages(tooling), "cargo-harmos")
    collected = []
    for package, manifest, backend, capture in [
        ("schematic-sidecar", SCHEMATIC, "sidecar", native),
        *((package, manifest, "guest", None) for package, manifest in GUESTS.items()),
    ]:
        if capture is None:
            capture = _compile(package, manifest.parent, profile, ["-p", package, "--target", "wasm32-unknown-unknown"])
        collected.append((package, _collect(tool, manifest, package, backend, profile, capture)))

    # Staging Starts From An Empty Fixture Directory, So The Stamp Names Only What This Build Produced.
    shutil.rmtree(FIXTURES, ignore_errors=True)
    FIXTURES.mkdir(parents=True)
    files: dict[str, str] = {}
    artifacts = {"cargo-harmos": _keep(tool, "cargo-harmos", files)}
    builds = {}
    for package, (build, receipt) in collected:
        artifacts[package], builds[package] = _stage_build(build, receipt, package, files)
    artifacts["schematic-cli"] = _keep(_executable(_messages(native), "schematic-cli"), "schematic-cli", files)
    # One Capture Supplies Every Library, So The Facade Links Exactly The Builds Beside It.
    libraries = _libraries(_messages(tooling), files)
    inputs = _offline_inputs(FIXTURES / "inputs")
    # Artifact Transfer Drops File Modes, So The Stamp Names The Staged Inputs Consumers Must Mark Executable Again.
    executables = sorted(
        path.relative_to(FIXTURES).as_posix() for path in inputs.rglob("*") if path.is_file() and path.stat().st_mode & stat.S_IXUSR
    )
    archive.stamp(
        ROOT,
        profile,
        sorted(files),
        {
            "artifacts": artifacts,
            "libraries": libraries,
            "builds": builds,
            "build_roots": {name: str(ROOT) for name in builds},
            "dependencies": "libraries",
            "vendor": "inputs/vendor",
            "wheels": "inputs/wheels",
            "requirements": "inputs/requirements.txt",
            "executables": [artifacts[name] for name in ("cargo-harmos", "schematic-cli", "schematic-sidecar")],
            "input_executables": executables,
        },
    )


# === Private Functions === #


def _compile(name: str, directory: Path, profile: str, extra: list[str]) -> dict:
    """Compile One Cargo Selection And Record The Collection Input `cargo harmos` Requires."""
    destination = CAPTURES / name / profile
    shutil.rmtree(destination, ignore_errors=True)
    destination.mkdir(parents=True)
    rustc = subprocess.check_output(["rustc", "-Vv"], cwd=directory, text=True)
    host = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    target = extra[extra.index("--target") + 1] if "--target" in extra else host
    platforms = [option for triple in dict.fromkeys((target, host)) for option in ("--filter-platform", triple)]
    features = ["--all-features"] if "--all-features" in extra else []
    metadata = destination / "metadata.json"
    with metadata.open("w", encoding="utf-8") as output:
        subprocess.run(
            ["cargo", "metadata", "--locked", "--format-version", "1", *platforms, *features], cwd=directory, stdout=output, check=True
        )
    messages = destination / "messages.json"
    command = ["cargo", "build", "--locked", "--profile", profile, "--message-format=json", *extra]
    with messages.open("w", encoding="utf-8") as output:
        if subprocess.run(command, cwd=directory, stdout=output, check=False).returncode:
            raise RuntimeError(f"cargo build failed for {name}")
    lines = [json.loads(line) for line in messages.read_text(encoding="utf-8").splitlines() if line.startswith("{")]
    produced = {
        path
        for message in lines
        if message.get("reason") == "compiler-artifact"
        for path in [*message["filenames"], *filter(None, [message.get("executable")])]
    }
    generated = {
        str(path)
        for message in lines
        if message.get("reason") == "build-script-executed"
        for path in Path(message["out_dir"]).rglob("*")
        if path.is_file()
    }
    record = {
        "schema": 1,
        "profile": profile,
        "target": target,
        "command": command,
        "inputs": {"rustc": rustc},
        "messages": {"path": str(messages), "sha256": _digest(messages)},
        "metadata": {"path": str(metadata), "sha256": _digest(metadata)},
        "artifacts": {name: _digest(Path(name)) for name in sorted(produced)},
        "generated": {name: _digest(Path(name)) for name in sorted(generated)},
    }
    receipt = destination / "capture.json"
    receipt.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    record["receipt"] = str(receipt)
    return record


def _messages(record: dict) -> list[dict]:
    content = Path(record["messages"]["path"]).read_text(encoding="utf-8")
    return [json.loads(line) for line in content.splitlines() if line.startswith("{")]


def _collect(tool: Path, manifest: Path, package: str, backend: str, profile: str, capture: dict) -> tuple[dict, Path]:
    destination = TARGET / "collected" / package
    shutil.rmtree(destination, ignore_errors=True)
    destination.mkdir(parents=True)
    result = destination / "build.json"
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
        capture["messages"]["path"],
        "--metadata-json",
        capture["metadata"]["path"],
        "--archive-json",
        capture["receipt"],
        "--json",
    ]
    with result.open("w", encoding="utf-8") as output:
        if subprocess.run(command, cwd=ROOT, stdout=output, check=False).returncode:
            raise RuntimeError(f"cargo harmos could not collect {package}")
    return json.loads(result.read_text(encoding="utf-8")), result


def _offline_inputs(destination: Path) -> Path:
    """Export The Locked Native Dependencies The Offline Authoring Stories Build From."""
    manifests = sorted({ROOT / "Cargo.toml", SCHEMATIC, *GUESTS.values()})
    vendor = ["cargo", "vendor", "--locked", "--versioned-dirs"]
    for manifest in manifests:
        if manifest != ROOT / "Cargo.toml":
            vendor += ["--sync", str(manifest)]
    vendor.append(str(destination / "vendor"))
    destination.mkdir(parents=True, exist_ok=True)
    subprocess.run(vendor, cwd=ROOT, stdout=subprocess.DEVNULL, check=True)
    projects = [ROOT / "tools/python", *(ROOT / f"crates/cargo-harmos/templates/{kind}/tools/python" for kind in ("sidecar", "guest"))]
    requirements = []
    for index, project in enumerate(projects):
        selected = destination / ("requirements.txt" if index == 0 else f"starter-{project.parents[1].name}.txt")
        with selected.open("w", encoding="utf-8") as output:
            export = ["uv", "export", "--locked", "--all-groups", "--no-emit-project", "--project", str(project)]
            subprocess.run(export, cwd=ROOT, stdout=output, check=True)
        requirements.append(selected)
    bundled = importlib.resources.files("ensurepip").joinpath("_bundled")
    wheels = [path for path in bundled.iterdir() if path.name.startswith("pip-") and path.name.endswith(".whl")]
    if len(wheels) != 1:
        raise RuntimeError("the pinned Python must provide its bundled pip wheel to stage offline inputs")
    launch = "import runpy,sys; sys.path.insert(0,sys.argv.pop(1)); runpy.run_module('pip',run_name='__main__')"
    # Each Project Locks Its Own Closure, So Each Downloads Separately Into The Shared Wheel Directory.
    download = [sys.executable, "-c", launch, str(wheels[0]), "download", "--dest", str(destination / "wheels"), "--require-hashes"]
    for selected in requirements:
        subprocess.run([*download, "--requirement", str(selected)], cwd=ROOT, check=True)
    # Hatchling Requests `editables` Only For Editable Installs, So No Project Lock Names It.
    subprocess.run([*download, "--no-deps", "--requirement", str(ROOT / "tools/python/editables.txt")], cwd=ROOT, check=True)
    return destination


def _stage_build(build: dict, collected: Path, package: str, files: dict) -> tuple[str, str]:
    selected = [artifact for artifact in build["artifacts"] if artifact["producer"] == package]
    if len(selected) != 1:
        raise RuntimeError(f"{package} did not produce exactly one artifact: {selected}")
    artifact = selected[0]
    root = FIXTURES / "builds" / package
    source = Path(artifact["path"])
    destination = root / source.relative_to(ROOT)
    if _copy(source, destination, files) != artifact["sha256"]:
        raise RuntimeError("typed artifact digest differs from its collected build")
    for schema in build["schemas"]:
        source = Path(schema["directory"])
        for name, expected in schema["files"].items():
            original = source / name
            if _copy(original, root / original.relative_to(ROOT), files) != expected:
                raise RuntimeError("typed schema digest differs from its collected build")
    # Original Build JSON Remains Byte-Exact; Packaging Relocates Its Typed Paths.
    receipt = FIXTURES / "receipts" / f"{package}.json"
    _copy(collected, receipt, files)
    return destination.relative_to(FIXTURES).as_posix(), receipt.relative_to(FIXTURES).as_posix()


def _copy(source: Path, destination: Path, files: dict) -> str:
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    digest = _digest(destination)
    files[destination.relative_to(ROOT).as_posix()] = digest
    return digest


def _digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


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


def _libraries(messages: list[dict], files: dict) -> dict[str, str]:
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
                _copy(path, destination / path.name, files)
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


def _keep(source: Path, name: str, files: dict) -> str:
    destination = FIXTURES / name / source.name
    _copy(source, destination, files)
    return destination.relative_to(FIXTURES).as_posix()


# === Entry Point === #

if __name__ == "__main__":
    main()
