# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Conformance."""

# === Imports === #

# Standard Library Imports
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.package.run
import harmos_tools.release.source

from .. import temporary
from . import layout, policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def verify(directory: Path, settings: dict, profile: str, target: str, root: Path) -> None:
    """Run The Downloaded Reference Host With Its Sibling Reference Plugin."""
    name = layout.conformance_name(directory, settings, target, harmos_tools.release.source.release_tag(directory)[1:])
    source = root / target / name
    if not source.is_file():
        source = root / name
    suffix = ".exe" if "-windows-" in layout.triple(directory, settings, target) else ""
    install_directory = temporary.Directory()
    install = install_directory.path
    try:
        for binary in ("harmos-conformance", "harmos-conformance-plugin"):
            destination = install / (binary + suffix)
            destination.write_bytes(harmos_tools.package.run.binary_bytes(source, destination.name))
            destination.chmod(0o755)
        policy.execute(install, [str(install / ("harmos-conformance" + suffix)), "--help"])
    finally:
        install_directory.close()


# === Private Functions === #

# === Entry Point === #
