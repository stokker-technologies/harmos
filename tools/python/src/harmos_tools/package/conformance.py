# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Package The Reference Host And Plugin From Matching Product Outputs"""

# === Imports === #

# Standard Library Imports
import tarfile
import zipfile
from pathlib import Path

# Third Party Imports
# Local Imports
from ..release import layout, policy, sbom
from . import run as packages

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def package(directory: Path, profile: str, target: str) -> None:
    """Package The Reference Host And Plugin From Matching Product Bytes, Without Compilation."""
    settings = policy.policy(directory, "release")
    version = policy.policy(directory, "version")["value"]
    build = directory / "target/build" / profile / target
    abi = layout.triple(directory, settings, target)
    actual = packages.regular(build, "product/target.txt").read_text().strip()
    if actual != abi:
        raise policy.ToolError(f"Build Rust target {actual} differs from release target {abi}")
    output = directory / "target/package" / profile / target
    destination = output / "rust" / layout.conformance_name(directory, settings, target, version)
    destination.parent.mkdir(parents=True, exist_ok=True)
    suffix = ".exe" if "-windows-" in abi else ""
    inputs = [
        (packages.regular(build, f"bin/{name}{suffix}"), name + suffix) for name in ("harmos-conformance", "harmos-conformance-plugin")
    ]
    inputs.append((packages.regular(directory, "LICENSE"), "LICENSE"))
    if suffix:
        with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for source, name in inputs:
                archive.write(source, name)
    else:
        with tarfile.open(destination, "w:gz") as archive:
            for source, name in inputs:
                entry = archive.gettarinfo(str(source), arcname=name)
                entry.mode = 0o755 if name.startswith("harmos-conformance") else 0o644
                with source.open("rb") as stream:
                    archive.addfile(entry, stream)
    sbom.generate(directory, abi, output / "compliance")


# === Private Functions === #

# === Entry Point === #
