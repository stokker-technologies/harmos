# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Join Product And Tests Fixture Manifests Without Compilation"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os

# Third Party Imports

# Local Imports
from ...build import fixtures as staging
from ...build import paths
from ...build.fixtures import ROOT
from ...test import archive

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    prepare(parser.parse_args().profile)


def prepare(profile: str) -> None:
    """Join Independently Exported Product And Tests Inputs Without Compilation."""
    export = paths.output(ROOT, "build", profile)
    fixtures = export / "fixtures"
    stamp = json.loads((fixtures / "stamp.json").read_text(encoding="utf-8"))
    staging.verify(stamp["profile"], profile, "test fixtures belong to another profile")
    staging.verify(stamp["target"], paths.target(), "test fixtures belong to another native target")
    catalog = json.loads((export / "product/examples/catalog.json").read_text(encoding="utf-8"))
    files = {**stamp["files"], **catalog["files"]}
    for relative, expected in files.items():
        staging.verify(staging.digest(ROOT / relative), expected, f"fixture bytes differ from their producer: {relative}")

    metadata = stamp["metadata"]
    for key in ("artifacts", "builds", "libraries"):
        metadata[key].update({name: os.path.relpath(export / path, fixtures) for name, path in catalog[key].items()})
    metadata["build_roots"].update(catalog["build_roots"])
    metadata["dependencies"] = "../product/examples/libraries"
    metadata["executables"] = [metadata["artifacts"][name] for name in ("cargo-harmos", "schematic-cli", "schematic-sidecar")]
    # Integration Inspects Immutable Copies Without Running Product Hosts.
    for name in ("schematic-sidecar", "schematic-guest"):
        source = export / catalog["artifacts"][name]
        destination = fixtures / "inspection" / source.name
        staging.copy(source, destination, files)
        metadata["inspections"][name] = destination.relative_to(fixtures).as_posix()
    archive.stamp(ROOT, profile, sorted(files), metadata)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
