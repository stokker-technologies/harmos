# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Requirements."""

# === Imports === #

# Standard Library Imports
import json
import os
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.release.crates
import harmos_tools.release.source

from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def refuse_overrides(directory: Path) -> None:
    """Published Bytes Never Come From Unpublished Local Dependency Checkouts."""
    if not (directory / "Cargo.toml").is_file():
        return
    # Read Structured Input
    text = policy.execute(directory, ["cargo", "metadata", "--locked", "--format-version", "1"], capture=True)
    metadata = json.loads(text)
    outside = [
        package["name"]
        for package in metadata["packages"]
        if package["source"] is None and not Path(package["manifest_path"]).resolve().is_relative_to(directory.resolve())
    ]
    if outside:
        raise policy.ToolError(f"unpublished local dependency override: {', '.join(sorted(outside))}")


def require_publication(directory: Path, settings: dict) -> None:
    """Refuse A Release Whose Crates Cannot Publish: A First Publication Needs The Bootstrap Token."""
    if not (directory / "Cargo.toml").is_file():
        return
    missing = [name for name in harmos_tools.release.crates.publishable(directory) if not harmos_tools.release.crates.published(name)]
    if not missing:
        return
    available = os.environ.get("CRATES_BOOTSTRAP_AVAILABLE") == "true" or bool(os.environ.get("CARGO_REGISTRY_TOKEN"))
    message = f"crates not yet on crates.io need the bootstrap token for their first publication: {', '.join(missing)}"
    if harmos_tools.release.source.continuous_integration() and not available:
        raise policy.ToolError(message + "; configure the [release] crates_bootstrap_secret environment secret before preparing")
    if not available:
        print("warning: " + message)


# === Private Functions === #

# === Entry Point === #
