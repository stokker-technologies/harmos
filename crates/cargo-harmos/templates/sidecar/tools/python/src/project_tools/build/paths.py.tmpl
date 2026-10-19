# Rendered by stkr project update from templates/python/build/paths.py; sha256:7f5f60a990e148eeb825c8c473cb28d49c8e7ce1f1e95efeee91b28b3028b970
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Resolve Task Outputs By Profile And Native Target"""

# === Imports === #

# Standard Library Imports
import argparse
import platform
import re
import shutil
from pathlib import Path

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

STAGES = ("build", "package", "tests")
PROFILES = ("dev", "release", "bench")

# === Types === #

# === Classes === #

# === Public Functions === #


def target() -> str:
    system = {"Linux": "linux", "Windows": "windows", "Darwin": "macos"}.get(platform.system())
    machine = platform.machine().lower()
    architecture = {"amd64": "x86_64", "arm64": "aarch64"}.get(machine, machine)
    if system is None or re.fullmatch(r"[a-z0-9_]+", architecture) is None:
        raise ValueError("unsupported native artifact target")
    return f"{system}-{architecture}"


def output(directory: Path, stage: str, profile: str, native: str | None = None) -> Path:
    if stage not in STAGES or profile not in PROFILES or profile == "bench" and stage != "build":
        raise ValueError("unknown task stage or profile")
    native = target() if native is None else native
    if re.fullmatch(r"(?:linux|windows|macos)-[a-z0-9_]+|any", native) is None:
        raise ValueError("invalid artifact target")
    result = directory / "target" / stage / profile / native
    if not result.resolve().is_relative_to(directory.resolve()):
        raise ValueError("task output escapes the repository")
    return result


def replace(directory: Path, source: Path, destination: Path, *, origin: Path | None = None) -> None:
    """Replace One Owned Output Subtree, Preserving Other Profiles And Targets."""
    root = directory.resolve()
    source_root = root if origin is None else origin.resolve()
    if not source.resolve().is_relative_to(source_root) or not destination.resolve().is_relative_to(root):
        raise ValueError("artifact path escapes the repository")
    overlap = source.resolve().is_relative_to(destination.resolve()) or destination.resolve().is_relative_to(source.resolve())
    if destination.is_symlink() or overlap:
        raise ValueError("artifact source overlaps its destination")
    if not source.is_dir():
        raise ValueError(f"missing built artifacts: {source}")
    for path in source.rglob("*"):
        if path.is_symlink() and not path.resolve().is_relative_to(source_root):
            raise ValueError(f"artifact link escapes the repository: {path}")
    if destination.exists():
        shutil.rmtree(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, destination)


def main() -> None:
    parser = argparse.ArgumentParser(description="Resolve A Nominal Task Output")
    parser.add_argument("stage", choices=STAGES)
    parser.add_argument("--profile", choices=PROFILES, default="dev")
    parser.add_argument("--target")
    options = parser.parse_args()
    print(output(Path.cwd(), options.stage, options.profile, options.target))


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
