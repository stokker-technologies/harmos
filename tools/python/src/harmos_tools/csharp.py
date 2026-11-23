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
import subprocess
from argparse import ArgumentParser
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


def run(root: Path, command: str, profile: str = "dev", *, check: bool = False, layer: str | None = None) -> int:
    arguments = ["mise", "-C", str(root / "sdk/csharp"), "run", command, "--"]
    if command in ("build", "test", "package"):
        arguments += ["--profile", profile]
    if check:
        arguments += ["--check"]
    if layer:
        arguments += ["--layer", layer]
    return subprocess.run(arguments, cwd=root, check=False, creationflags=spawn_flags()).returncode


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "test", "format", "check", "package"))
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--layer", choices=("unit", "integration"))
    options = parser.parse_args()
    raise SystemExit(run(Path(__file__).resolve().parents[4], options.command, options.profile, check=options.check, layer=options.layer))


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
