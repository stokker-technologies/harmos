# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Assemble Rust Crate Packages And Already Built Native Artifacts"""

# === Imports === #

# Standard Library Imports
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

# Third Party Imports

# Local Imports
from ..build.fixtures import ROOT
from ..build import paths
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

    # Cargo Packages Source Without Compiling The Candidate.
    subprocess.run(
        [sys.executable, "-m", run.__name__, "rust", "--profile", profile],
        cwd=ROOT,
        check=True,
    )
    assets = {f"rust/{source.name}": _asset(source, _digest(source)) for source in (destination / "rust").glob("*.crate")}

    # Distribute The Exact Product Bytes.
    for name in ("cargo-harmos", "schematic-cli"):
        source = export / selectors["artifacts"][name]
        expected = selectors["files"][source.relative_to(ROOT).as_posix()]
        _copy(source, destination / source.name, expected)
        assets[source.name] = _asset(destination / source.name, expected)
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
        with result.open("w", encoding="utf-8") as output:
            subprocess.run(command, cwd=ROOT, stdout=output, check=True)
        build = json.loads(receipt.read_text(encoding="utf-8"))
        for artifact in build["artifacts"]:
            path = destination / artifact["producer"] / f"{artifact['name']}.harmos"
            assets[path.relative_to(destination).as_posix()] = {"sha256": _digest(path), "bytes": path.stat().st_size}
    catalog = {"profile": profile, "assets": assets}
    (destination / "catalog.json").write_text(json.dumps(catalog, indent=2) + "\n", encoding="utf-8")


# === Private Functions === #


def _copy(source: Path, destination: Path, expected: str) -> None:
    shutil.copy2(source, destination)
    _asset(destination, expected)


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
