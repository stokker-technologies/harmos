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
import harmos_tools.package.run

from ..release import layout, policy, sbom

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
    actual = harmos_tools.package.run.regular(build, "product/target.txt").read_text().strip()
    if actual != abi:
        raise policy.ToolError(f"Build Rust target {actual} differs from release target {abi}")
    output = directory / "target/package" / profile / target
    destination = output / "rust" / layout.conformance_name(directory, settings, target, version)
    destination.parent.mkdir(parents=True, exist_ok=True)
    suffix = ".exe" if "-windows-" in abi else ""
    inputs = [
        (harmos_tools.package.run.regular(build, f"bin/{name}{suffix}"), name + suffix)
        for name in ("harmos-conformance", "harmos-conformance-plugin")
    ]
    inputs.append((harmos_tools.package.run.regular(directory, "LICENSE"), "LICENSE"))
    if suffix:
        archive = zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED)
        try:
            for source, name in inputs:
                archive.write(source, name)
        finally:
            archive.close()
    else:
        archive = tarfile.open(destination, "w:gz")
        try:
            for source, name in inputs:
                entry = archive.gettarinfo(str(source), arcname=name)
                entry.mode = 0o755 if name.startswith("harmos-conformance") else 0o644
                stream = source.open("rb")
                try:
                    archive.addfile(entry, stream)
                finally:
                    stream.close()
        finally:
            archive.close()
    sbom.generate(directory, abi, output / "compliance")


# === Private Functions === #

# === Entry Point === #
