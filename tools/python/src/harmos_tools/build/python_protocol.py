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
PROTOCOLS = tuple(
    sorted(
        path.relative_to(ROOT / "proto").as_posix()
        for face in ("v1", "context/v1", "plugin/v1")
        for path in (ROOT / "proto/harmos" / face).rglob("*.proto")
        if path.name != "guest.proto"
    )
) + ("harmos/conformance/v1/fixture.proto",)
GENERATED = tuple(name.removesuffix(".proto") + suffix for name in PROTOCOLS for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py"))

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
        f"-I{Path(grpc_tools.__file__).parent / '_proto'}",
        f"--python_out={output}",
        f"--pyi_out={output}",
        f"--grpc_python_out={output}",
        *PROTOCOLS,
    ]
    subprocess.run(command, check=True, cwd=ROOT, creationflags=spawn_flags())
    # Generated Convenience Calls Require the Experimental Submodule Explicitly.
    for name in GENERATED:
        if name.endswith("_pb2_grpc.py"):
            path = output / name
            source = path.read_text(encoding="utf-8")
            if "grpc.experimental." in source:
                path.write_text(
                    source.replace("import grpc\n", "import grpc\nimport grpc.experimental\n", 1), encoding="utf-8", newline="\n"
                )


def generate_client_fixture(output: Path) -> None:
    proto = ROOT / "sdk/fixtures/client/proto"
    inputs = sorted(path.relative_to(proto).as_posix() for path in proto.rglob("*.proto"))
    output.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [sys.executable, "-m", "grpc_tools.protoc", f"-I{proto}", f"--python_out={output}", f"--grpc_python_out={output}", *inputs],
        check=True,
        cwd=ROOT,
        creationflags=spawn_flags(),
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    committed = ROOT / "sdk/python/src"
    if not options.check:
        generate(committed)
        generate_client_fixture(ROOT / "sdk/python/tests/fixtures/client")
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
