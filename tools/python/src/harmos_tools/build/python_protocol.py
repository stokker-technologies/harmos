# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Generate and Compare the Python Protocol Bindings."""

# === Imports === #

# Standard Library Imports
import argparse
import filecmp
import subprocess
import sys
import tempfile
from pathlib import Path

# Third Party Imports
import grpc_tools

# Local Imports
from ..process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[5]
GENERATED = (
    "harmos/v1/plugin_pb2.py",
    "harmos/v1/plugin_pb2.pyi",
    "harmos/v1/plugin_pb2_grpc.py",
    "harmos/conformance/v1/fixture_pb2.py",
    "harmos/conformance/v1/fixture_pb2.pyi",
    "harmos/conformance/v1/fixture_pb2_grpc.py",
)

# === Types === #

# === Classes === #

# === Public Functions === #


def generate(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    command = [
        sys.executable,
        "-m",
        "grpc_tools.protoc",
        f"-I{ROOT / 'proto'}",
        f"-I{ROOT / 'sdk/python/proto'}",
        f"-I{Path(grpc_tools.__file__).parent / '_proto'}",
        f"--python_out={output}",
        f"--pyi_out={output}",
        f"--grpc_python_out={output}",
        "harmos/v1/plugin.proto",
        "harmos/conformance/v1/fixture.proto",
    ]
    subprocess.run(command, check=True, cwd=ROOT, creationflags=spawn_flags())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    committed = ROOT / "sdk/python/src"
    if not options.check:
        generate(committed)
        return

    with tempfile.TemporaryDirectory() as scratch:
        output = Path(scratch)
        generate(output)
        changed = [
            name for name in GENERATED if not (committed / name).exists() or not filecmp.cmp(output / name, committed / name, shallow=False)
        ]
        if changed:
            raise SystemExit(f"Generated Python Differs: {', '.join(changed)}; Run mise run generate -- --sdk python")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
