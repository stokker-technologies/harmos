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
import hashlib
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

ROOT = Path(__file__).resolve().parents[5]
TARGET = ROOT / "target"
FIXTURES = TARGET / "fixtures"
STAMP = FIXTURES / "stamp.json"
SCHEMATIC = ROOT / "examples/schematic/Cargo.toml"
GUESTS = {"schematic-guest": SCHEMATIC, "harmos-sandbox-guest": ROOT / "crates/harmos-guest/tests/fixtures/sandbox/Cargo.toml"}
INPUTS = ("Cargo.toml", "Cargo.lock", "crates", "examples")
LIBRARIES = ("harmos", "harmos_sidecar", "harmos_macros", "serde")

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    workspace = _cargo(["build", "--locked", "--workspace", "--all-targets", "--all-features"])
    _cargo(["build", "--locked", "--manifest-path", str(SCHEMATIC), "--workspace", "--target-dir", str(TARGET)])
    tool = _executable(workspace, "cargo-harmos")
    shutil.rmtree(FIXTURES, ignore_errors=True)
    FIXTURES.mkdir(parents=True)

    artifacts = {}
    for package, manifest in GUESTS.items():
        built = _guest(tool, manifest, package)
        paths = [Path(item["path"]) for item in built["artifacts"] if item["producer"] == package]
        if len(paths) != 1:
            raise RuntimeError(f"{package} did not produce exactly one component: {paths}")
        artifacts[package] = _keep(paths[0], package)
    for name in ("schematic-sidecar", "schematic-cli"):
        artifacts[name] = _keep(TARGET / "debug" / (name + (".exe" if os.name == "nt" else "")), name)

    stamp = {
        "inputs": digest(),
        "artifacts": artifacts,
        "libraries": _libraries(workspace),
        "dependencies": os.path.relpath(TARGET / "debug/deps", FIXTURES),
    }
    STAMP.write_text(json.dumps(stamp, indent=2) + "\n", encoding="utf-8")


def digest() -> str:
    """The content digest of every tracked or new source file the build reads."""
    listed = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", *INPUTS],
        cwd=ROOT,
        check=True,
        capture_output=True,
    ).stdout
    hasher = hashlib.sha256()
    for name in sorted(set(listed.decode().split("\0")) - {""}):
        path = ROOT / name
        if path.is_file():
            hasher.update(name.encode() + b"\0" + path.read_bytes() + b"\0")
    return hasher.hexdigest()


# === Private Functions === #


def _cargo(arguments: list[str]) -> list[dict]:
    command = [os.environ.get("CARGO", "cargo"), *arguments, "--message-format=json-render-diagnostics"]
    result = subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.PIPE, text=True)
    return [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]


def _guest(tool: Path, manifest: Path, package: str) -> dict:
    command = [str(tool), "harmos", "guest", "build", "--manifest-path", str(manifest), "-p", package]
    command += ["--target-dir", str(TARGET), "--json"]
    result = subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.PIPE, text=True)
    return json.loads(result.stdout)


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
    """The exact rlibs and proc-macro the compile-fail stories link, by the debug-info target profile."""
    found: dict[str, set[str]] = {name: set() for name in LIBRARIES}
    for message in messages:
        name = message.get("target", {}).get("name")
        if message.get("reason") != "compiler-artifact" or name not in found:
            continue
        if not message["profile"]["debuginfo"] and name != "harmos_macros":
            continue
        suffix = (".dll" if os.name == "nt" else ".dylib" if sys.platform == "darwin" else ".so") if name == "harmos_macros" else ".rlib"
        found[name].update(path for path in message["filenames"] if path.endswith(suffix))
    ambiguous = {name: sorted(paths) for name, paths in found.items() if len(paths) != 1}
    if ambiguous:
        raise RuntimeError(f"compiler libraries are missing or ambiguous: {ambiguous}")
    return {name: os.path.relpath(paths.pop(), FIXTURES) for name, paths in found.items()}


def _keep(source: Path, name: str) -> str:
    destination = FIXTURES / name / source.name
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    return destination.relative_to(FIXTURES).as_posix()


# === Entry Point === #

if __name__ == "__main__":
    main()
