# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Build Benchmarks and Their Matching Schematic Guest Fixture."""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
from pathlib import Path

# Third Party Imports

# Local Imports
from .. import temporary
from ..test import archive
from . import bench, fixtures, paths

# Type Checking Imports


# === Public Exports === #

# === Constants === #

FEATURES = "harmos-runtime/plugin-grpc,harmos-runtime/plugin-guest,harmos-runtime/grpc"

# === Types === #

# === Classes === #

# === Public Functions === #


def build(extra: list[str]) -> Path:
    root = fixtures.ROOT
    os.environ["CARGO_TARGET_DIR"] = str(fixtures.TARGET)
    arguments = ["--features", FEATURES, *extra]
    messages = bench.build(root, arguments)
    output = paths.output(root, "build", "bench")
    directory = output / "fixtures"
    stamp = json.loads((directory / "stamp.json").read_text(encoding="utf-8"))
    files = dict(stamp["files"])
    artifacts = {}

    with temporary.directory() as scratch:
        tool = fixtures.driver("bench", scratch)
        for package, backend, target in (("schematic-guest", "guest", ["--features", "benchmark", "--target", "wasm32-unknown-unknown"]),):
            capture = fixtures.capture(package, fixtures.SCHEMATIC.parent, "bench", ["-p", package, *target], scratch)
            built, receipt = fixtures.collect(tool, fixtures.SCHEMATIC, package, backend, "bench", capture, scratch)
            artifact, _ = fixtures.stage_build(built, receipt, package, files, directory)
            artifacts[package] = artifact
    archive.stamp(root, "bench", list(files), {"artifacts": artifacts, "cargo_arguments": arguments})
    return messages


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("extra", nargs=argparse.REMAINDER)
    options = parser.parse_args()
    extra = options.extra[1:] if options.extra[:1] == ["--"] else options.extra
    print(build(extra))


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
