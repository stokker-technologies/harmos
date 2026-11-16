# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Publish."""

# === Imports === #

# Standard Library Imports
import urllib.parse
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.release.assets
import harmos_tools.release.requirements
import harmos_tools.release.source

from ..package.changelog import notes
from . import layout, pins, policy, registry, version

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(directory: Path, settings: dict) -> None:
    """Stage And Publish Exactly The Manifest's Downloads From The Protected Release Tag."""
    tag = harmos_tools.release.source.release_tag(directory)
    sha = harmos_tools.release.source.revision(directory)
    version.run(directory, "check")
    registry.require_tag(directory, tag)
    pins.check(directory)
    harmos_tools.release.requirements.refuse_overrides(directory)
    targets = list(settings.get("targets", []))
    if not targets:
        raise policy.ToolError(
            "release.targets must declare the required OS/architecture targets; "
            "Inspect the protected release revision and staged inventory before publishing"
        )
    root = harmos_tools.release.assets.staging_root(directory)
    harmos_tools.release.assets.stage(directory, settings, tag, targets)
    files = layout.uploads(directory, staged=True)
    state = {"sha": sha, "tag": tag, "name": tag, "files": {}}
    if settings["provider"] == "github":
        release = registry.draft(settings, tag, sha, notes(directory, tag, tagged=True))
        state["release_id"] = (release or {})["id"]
        for item in files:
            data = harmos_tools.release.assets.read(directory, item)
            state["files"][item["name"]] = {**item, "endpoint": registry.upload(settings, (release or {})["id"], item, data)}
            policy.write_json(root / harmos_tools.release.assets.SHARED / harmos_tools.release.assets.STATE, state)
    else:
        if registry.document(settings, "releases/" + urllib.parse.quote(tag, safe="")):
            raise policy.ToolError("published release cannot be replaced; use a new patch version")
        registry.verify_duplicate_policy(settings)
        for item in files:
            state["files"][item["name"]] = {
                **item,
                **registry.upload_generic(
                    settings, harmos_tools.release.assets.package(settings), tag, item, harmos_tools.release.assets.read(directory, item)
                ),
            }
            policy.write_json(root / harmos_tools.release.assets.SHARED / harmos_tools.release.assets.STATE, state)
    policy.write_json(root / harmos_tools.release.assets.SHARED / harmos_tools.release.assets.STATE, state)


# === Private Functions === #

# === Entry Point === #
