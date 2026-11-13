# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build and Prove the C# Plugin SDK Through the Shared Reference Host."""

# === Imports === #

# Standard Library Imports
import argparse
import subprocess
from pathlib import Path

# Third Party Imports
# Local Imports
from .process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def executable(root: Path, profile: str = "dev") -> Path:
    configuration = "Release" if profile == "release" else "Debug"
    manifest = root / "sdk/csharp/artifacts/paths" / configuration / "Harmos.Conformance.txt"
    if not manifest.is_file():
        raise FileNotFoundError("Build the C# Fixture: mise run build -- --language csharp")
    return root / "sdk/csharp" / manifest.read_text(encoding="utf-8-sig").strip()


def clients(root: Path, *, check: bool = False) -> None:
    from google.protobuf.descriptor_pb2 import FileDescriptorSet

    from .build.resource_roots import csharp, table

    descriptors = FileDescriptorSet.FromString((root / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
    methods = [method for method in table(descriptors) if method["face"] == "client"]
    csharp(root, methods, check=check)
    fixture = FileDescriptorSet.FromString((root / "sdk/rust/tests/fixtures/client/client-descriptors.bin").read_bytes())
    csharp(
        root,
        methods + table(fixture),
        namespace="fixture",
        target=root / "sdk/csharp/tests/Harmos.Tests/Generated/Client.g.cs",
        check=check,
    )


def run(root: Path, command: str, profile: str = "dev", *, check: bool = False, layer: str | None = None) -> int:
    if command in ("generate", "check-generated"):
        clients(root, check=command == "check-generated")
        return 0
    arguments = ["mise", "-C", str(root / "sdk/csharp"), "run", command, "--"]
    if command in ("build", "test", "package"):
        arguments += ["--profile", profile]
    if check:
        arguments += ["--check"]
    if layer:
        arguments += ["--layer", layer]
    return subprocess.run(arguments, cwd=root, check=False, creationflags=spawn_flags()).returncode


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "test", "format", "check", "package", "generate", "check-generated"))
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--layer", choices=("unit", "integration"))
    options = parser.parse_args()
    raise SystemExit(run(Path(__file__).resolve().parents[4], options.command, options.profile, check=options.check, layer=options.layer))


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
