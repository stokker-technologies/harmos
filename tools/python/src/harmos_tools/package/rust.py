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
import os
import shutil
import subprocess
import sys
import zipfile
from collections.abc import Iterable
from pathlib import Path
from typing import TypedDict, cast

# Third Party Imports
# Local Imports
from .. import temporary
from ..build import paths
from ..build.fixtures import ROOT
from ..process import spawn_flags
from . import conformance, run

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ReopenedNet(TypedDict):
    route: list[object]


class ReopenedDocument(TypedDict):
    symbols: dict[str, object]
    nets: dict[str, ReopenedNet]


class WalkthroughResult(TypedDict):
    document: ReopenedDocument


# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    profile = parser.parse_args().profile
    export = paths.output(ROOT, "build", profile)
    examples = export / "product/examples"
    catalog_path = examples / "catalog.json"
    # Read Structured Input
    text = catalog_path.read_text(encoding="utf-8")
    selectors = json.loads(text)
    for relative, expected in selectors["files"].items():
        _asset(ROOT / relative, expected)
    destination = paths.output(ROOT, "package", profile)
    shutil.rmtree(destination, ignore_errors=True)
    destination.mkdir(parents=True)

    # Assemble Declared Products From The Matching Build.
    # Run the Native Command
    command = [sys.executable, "-m", run.__name__, "rust", "--profile", profile]
    subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        creationflags=spawn_flags(),
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
    )
    conformance.package(ROOT, profile, paths.target())
    packaged = (source for source in (destination / "rust").iterdir() if source.is_file())
    assets = _records(packaged, destination)
    products = run.definitions(ROOT, profile, paths.target())
    bundles = [item for item in products if "triple" not in item]
    for bundle in bundles:
        _verify_schematic(destination / "rust" / bundle["output"], export, selectors)

    tool = export / "product/tools" / ("fixture-driver.exe" if sys.platform == "win32" else "fixture-driver")
    tool.chmod(tool.stat().st_mode | 0o111)
    for name in ("schematic-sidecar", "schematic-guest"):
        receipt = export / selectors["builds"][name]
        result = destination / f"{name}.json"
        request = {
            "action": "package",
            "build": str(receipt),
            "from": selectors["build_roots"][name],
            "to": str(examples / "builds" / name),
            "output": str(destination),
        }
        # Run the Native Command
        command = [str(tool)]
        output = subprocess.check_output(
            command,
            input=json.dumps(request),
            text=True,
            cwd=ROOT,
            creationflags=spawn_flags(),
            stderr=sys.stderr,
        )
        result.write_text(output, encoding="utf-8")
        # Read Structured Input
        text = receipt.read_text(encoding="utf-8")
        build = json.loads(text)
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

    install_directory = temporary.Directory()
    install = install_directory.path
    try:
        _unpack_schematic(bundle, install, expected)
        executable = install / ("schematic-cli.exe" if sys.platform == "win32" else "schematic-cli")
        _walkthrough(executable, install / "sidecar", [])
        _walkthrough(executable, install / "guest", ["--guest"])
    finally:
        install_directory.close()


def _unpack_schematic(bundle: Path, install: Path, expected: dict[str, str]) -> None:
    archive = zipfile.ZipFile(bundle)
    try:
        names = archive.namelist()
        contents = {name: archive.read(name) for name in expected}
    finally:
        archive.close()
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
    reports = directory.parent / f"{directory.name}-reports"
    # Run the Native Command
    command = [str(executable), *arguments]
    result = subprocess.run(
        command,
        env=dict(os.environ, STKR_TEST_REPORT_DIR=str(reports)),
        cwd=directory,
        capture_output=True,
        text=True,
        check=False,
        creationflags=spawn_flags(),
        stdin=sys.stdin,
    )
    if result.returncode:
        raise RuntimeError(f"Run Packaged Schematic {directory.name}: exit {result.returncode}\n{result.stdout}{result.stderr}")
    (report,) = reports.glob("schematic-walkthrough-*.json")
    record = cast(WalkthroughResult, json.loads(report.read_text(encoding="utf-8")))
    document = record["document"]
    if len(document["symbols"]) != 2 or len(document["nets"]) != 1 or len(document["nets"]["1"]["route"]) != 3:
        raise RuntimeError(f"Reopen Packaged Schematic {directory.name}: saved document differs from the walkthrough")


def _records(sources: Iterable[Path], destination: Path) -> dict:
    records = {}
    for path in sources:
        records[path.relative_to(destination).as_posix()] = {"sha256": _digest(path), "bytes": path.stat().st_size}
    return records


def _digest(path: Path) -> str:
    stream = path.open("rb")
    try:
        return hashlib.file_digest(stream, "sha256").hexdigest()
    finally:
        stream.close()


def _asset(path: Path, expected: str) -> dict:
    if _digest(path) != expected:
        raise RuntimeError(f"distribution bytes differ from the tested fixture: {path}")
    return {"sha256": expected, "bytes": path.stat().st_size}


# === Entry Point === #

if __name__ == "__main__":
    main()
