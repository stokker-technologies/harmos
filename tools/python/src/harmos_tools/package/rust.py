# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Assemble Declared Native Distributions And Typed Plugin Packages"""

# === Imports === #

# Standard Library Imports
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path
from collections.abc import Iterable

# Third Party Imports

# Local Imports
from ..build.fixtures import ROOT
from ..build import paths
from .. import temporary
from . import run

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    profile = parser.parse_args().profile
    export = paths.output(ROOT, "build", profile)
    examples = export / "product/examples"
    catalog_path = examples / "catalog.json"
    selectors = json.loads(catalog_path.read_text(encoding="utf-8"))
    for relative, expected in selectors["files"].items():
        _asset(ROOT / relative, expected)
    destination = paths.output(ROOT, "package", profile)
    shutil.rmtree(destination, ignore_errors=True)
    destination.mkdir(parents=True)

    # Assemble Declared Products From The Matching Build.
    subprocess.run(
        [sys.executable, "-m", run.__name__, "rust", "--profile", profile],
        cwd=ROOT,
        check=True,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
    )
    packaged = (source for source in (destination / "rust").iterdir() if source.is_file())
    assets = _records(packaged, destination)
    products = run.definitions(ROOT, profile, paths.target())
    bundles = [item for item in products if "triple" not in item]
    for bundle in bundles:
        _verify_schematic(destination / "rust" / bundle["output"], export, selectors)

    tool = export / selectors["artifacts"]["cargo-harmos"]
    for name, backend in (("schematic-sidecar", "sidecar"), ("schematic-guest", "guest")):
        receipt = export / selectors["builds"][name]
        result = destination / f"{name}.json"
        command = [
            str(tool),
            "harmos",
            backend,
            "package",
            "--build-json",
            str(receipt),
            "--relocate-from",
            selectors["build_roots"][name],
            "--relocate-to",
            str(examples / "builds" / name),
            "--output",
            str(destination),
            "--json",
        ]
        _package_plugin(command, result)
        build = json.loads(receipt.read_text(encoding="utf-8"))
        produced = [destination / artifact["producer"] / f"{artifact['name']}.harmos" for artifact in build["artifacts"]]
        records = _records(produced, destination)
        assets.update(records)
    catalog = {"profile": profile, "assets": assets}
    (destination / "catalog.json").write_text(json.dumps(catalog, indent=2) + "\n", encoding="utf-8")


# === Private Functions === #


def _verify_schematic(bundle: Path, export: Path, selectors: dict) -> None:
    expected = {}
    for name in ("schematic-cli", "schematic-sidecar", "schematic-guest"):
        source = export / selectors["artifacts"][name]
        relative = source.relative_to(export / "bin").as_posix()
        expected[relative] = selectors["files"][source.relative_to(ROOT).as_posix()]

    with temporary.directory() as install:
        _unpack_schematic(bundle, install, expected)
        executable = install / ("schematic-cli.exe" if sys.platform == "win32" else "schematic-cli")
        _walkthrough(executable, install / "sidecar", [])
        _walkthrough(executable, install / "guest", ["--guest"])


def _unpack_schematic(bundle: Path, install: Path, expected: dict[str, str]) -> None:
    with zipfile.ZipFile(bundle) as archive:
        names = archive.namelist()
        contents = {name: archive.read(name) for name in expected}
    if sorted(names) != sorted(expected):
        raise RuntimeError("Schematic bundle differs from its declared runtime layout")

    for name, data in contents.items():
        target = install / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        _asset(target, expected[name])
        permissions = 0 if name.endswith(".wasm") else 0o111
        target.chmod(target.stat().st_mode | permissions)


def _walkthrough(executable: Path, directory: Path, arguments: list[str]) -> None:
    directory.mkdir()
    result = subprocess.run(
        [str(executable), *arguments],
        cwd=directory,
        capture_output=True,
        text=True,
        check=True,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        stdin=sys.stdin,
    )
    if "Reopened: 2 symbols, 1 net, 3 route points" not in result.stdout.splitlines():
        raise RuntimeError(f"packaged Schematic {directory.name} walkthrough did not reopen its document")


def _package_plugin(command: list[str], result: Path) -> None:
    with result.open("w", encoding="utf-8") as output:
        subprocess.run(
            command,
            cwd=ROOT,
            stdout=output,
            check=True,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
            stdin=sys.stdin,
            stderr=sys.stderr,
        )


def _records(sources: Iterable[Path], destination: Path) -> dict:
    records = {}
    for path in sources:
        records[path.relative_to(destination).as_posix()] = {"sha256": _digest(path), "bytes": path.stat().st_size}
    return records


def _digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def _asset(path: Path, expected: str) -> dict:
    if _digest(path) != expected:
        raise RuntimeError(f"distribution bytes differ from the tested fixture: {path}")
    return {"sha256": expected, "bytes": path.stat().st_size}


# === Entry Point === #

if __name__ == "__main__":
    main()
