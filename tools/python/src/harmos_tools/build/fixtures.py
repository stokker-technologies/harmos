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
import json
import os
import shutil
import subprocess
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
    before = archive.tree(ROOT)
    workspace = _cargo(["build", "--locked", "--workspace", "--all-targets", "--all-features", "--profile", profile])
    schematic = _cargo([
        "build", "--locked", "--manifest-path", str(SCHEMATIC), "-p", "schematic-cli",
        "--target-dir", str(TARGET), "--profile", profile,
    ])
    tool = _executable(workspace, "cargo-harmos")
    shutil.rmtree(FIXTURES, ignore_errors=True)
    FIXTURES.mkdir(parents=True)

    artifacts = {"cargo-harmos": _keep(tool, "cargo-harmos")}
    builds = {}
    fixtures = [(package, manifest, "guest") for package, manifest in GUESTS.items()]
    fixtures.append(("schematic-sidecar", SCHEMATIC, "sidecar"))
    for package, manifest, backend in fixtures:
        built = _build(tool, manifest, package, backend, profile)
        staged, receipt = _stage_build(built, package)
        artifacts[package] = staged
        builds[package] = receipt
    artifacts["schematic-cli"] = _keep(_executable(schematic, "schematic-cli"), "schematic-cli")

    libraries = _libraries(workspace)
    if archive.tree(ROOT) != before:
        raise RuntimeError("sources changed while fixtures were building; rebuild the complete candidate")
    stamp = {
        "tree": before,
        "revision": archive.published(ROOT, before),
        "profile": profile,
        "artifacts": artifacts,
        "libraries": libraries,
        "builds": builds,
        "dependencies": "libraries",
        "executables": [artifacts[name] for name in ("cargo-harmos", "schematic-cli", "schematic-sidecar")],
        "hashes": {
            path.relative_to(FIXTURES).as_posix(): _digest(path)
            for path in FIXTURES.rglob("*") if path.is_file()
        },
    }
    STAMP.write_text(json.dumps(stamp, indent=2) + "\n", encoding="utf-8")
    if profile != "bench":
        archive.build(ROOT, profile, ["--all-features"])


# === Private Functions === #


def _digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def _cargo(arguments: list[str]) -> list[dict]:
    command = [os.environ.get("CARGO", "cargo"), *arguments, "--message-format=json-render-diagnostics"]
    result = subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.PIPE, text=True)
    return [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]


def _build(tool: Path, manifest: Path, package: str, backend: str, profile: str) -> dict:
    command = [str(tool), "harmos", backend, "build", "--manifest-path", str(manifest), "-p", package]
    command += ["--target-dir", str(TARGET), "--profile", profile, "--json"]
    result = subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.PIPE, text=True)
    return json.loads(result.stdout)


def _stage_build(build: dict, package: str) -> tuple[str, str]:
    selected = [artifact for artifact in build["artifacts"] if artifact["producer"] == package]
    if len(selected) != 1:
        raise RuntimeError(f"{package} did not produce exactly one artifact: {selected}")
    artifact = selected[0]
    relative = _keep(Path(artifact["path"]), package)
    artifact["path"] = str(FIXTURES / relative)
    for schema in build["schemas"]:
        source = Path(schema["directory"])
        destination = FIXTURES / "schemas" / schema["producer"] / schema["catalog"]
        shutil.copytree(source, destination)
        schema["directory"] = str(destination)
    receipt = FIXTURES / "receipts" / f"{package}.json"
    receipt.parent.mkdir(exist_ok=True)
    receipt.write_text(json.dumps(build, indent=2) + "\n", encoding="utf-8")
    return relative, receipt.relative_to(FIXTURES).as_posix()


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


def _libraries(messages: list[dict]) -> dict[str, str]:
    """Transfer exact compiler-emitted libraries needed by the authoring refusal stories."""
    found: dict[str, set[str]] = {name: set() for name in LIBRARIES}
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
            shutil.copy2(path, destination / path.name)
            if name not in found:
                continue
            suffix = (".dll" if os.name == "nt" else ".dylib" if sys.platform == "darwin" else ".so") if name == "harmos_macros" else ".rlib"
            if path.suffix == suffix:
                found[name].add((destination / path.name).relative_to(FIXTURES).as_posix())
    ambiguous = {name: sorted(paths) for name, paths in found.items() if len(paths) != 1}
    if ambiguous:
        raise RuntimeError(f"compiler libraries are missing or ambiguous: {ambiguous}")
    return {name: paths.pop() for name, paths in found.items()}


def _keep(source: Path, name: str) -> str:
    destination = FIXTURES / name / source.name
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    return destination.relative_to(FIXTURES).as_posix()


# === Entry Point === #

if __name__ == "__main__":
    main()
