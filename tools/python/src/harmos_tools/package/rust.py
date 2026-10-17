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
import tomllib
from pathlib import Path

# Third Party Imports

# Local Imports
from ..build.fixtures import FIXTURES, ROOT, STAMP
from ..test import preflight

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
    preflight.main(["--profile", profile])
    stamp = json.loads(STAMP.read_text(encoding="utf-8"))
    destination = ROOT / "dist" / profile
    shutil.rmtree(destination, ignore_errors=True)
    destination.mkdir(parents=True)

    # Assemble Packages Without Recompiling The Candidate
    subprocess.run(["cargo", "package", "--workspace", "--locked", "--no-verify"], cwd=ROOT, check=True)
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())
    version = workspace["workspace"]["package"]["version"]
    for manifest in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        package = tomllib.loads(manifest.read_text())["package"]
        if package.get("publish") is False:
            continue
        selected = package.get("version", version)
        selected = version if isinstance(selected, dict) else selected
        filename = f"{package['name']}-{selected}.crate"
        shutil.copy2(ROOT / "target/package" / filename, destination / filename)

    # Keep The Product Tool And The Shipped Example Beside Their Native Bundles
    for name in ("cargo-harmos", "schematic-cli"):
        source = FIXTURES / stamp["artifacts"][name]
        shutil.copy2(source, destination / source.name)
    tool = FIXTURES / stamp["artifacts"]["cargo-harmos"]
    for name, backend in (("schematic-sidecar", "sidecar"), ("schematic-guest", "guest")):
        receipt = FIXTURES / stamp["builds"][name]
        subprocess.run(
            [str(tool), "harmos", backend, "package", "--build-json", str(receipt), "--output", str(destination)],
            cwd=ROOT, check=True,
        )

    assets = {}
    for path in destination.rglob("*"):
        if not path.is_file():
            continue
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        assets[path.relative_to(destination).as_posix()] = {"sha256": digest, "bytes": path.stat().st_size}
    catalog = {"tree": stamp["tree"], "revision": stamp["revision"], "profile": profile, "assets": assets}
    (destination / "catalog.json").write_text(json.dumps(catalog, indent=2) + "\n", encoding="utf-8")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
