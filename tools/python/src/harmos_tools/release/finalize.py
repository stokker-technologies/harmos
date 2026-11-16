# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Finalize."""

# === Imports === #

# Standard Library Imports
import json
import urllib.parse
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.release.assets
import harmos_tools.release.source

from .. import temporary
from ..package.changelog import notes
from . import layout, policy, registry

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(directory: Path, settings: dict) -> None:
    """Verify Every Manifest Download And Signature Before Making The Release Visible."""
    root = harmos_tools.release.assets.staging_root(directory)
    # Read Structured Input
    text = (root / harmos_tools.release.assets.SHARED / harmos_tools.release.assets.STATE).read_text(encoding="utf-8")
    state = json.loads(text)
    if state.get("sha") != harmos_tools.release.source.revision(directory) or state.get("tag") != harmos_tools.release.source.release_tag(
        directory
    ):
        raise policy.ToolError("the staged publication belongs to another revision or version")
    inventory = {item["name"]: item for item in layout.uploads(directory, staged=True)}
    if set(inventory) != set(state["files"]) or any(
        any(entry.get(key) != value for key, value in inventory[name].items()) for name, entry in state["files"].items()
    ):
        raise policy.ToolError("publication state differs from upload.json")
    downloads_directory = temporary.Directory()
    downloads = downloads_directory.path
    try:
        for item in state["files"].values():
            (downloads / item["name"]).write_bytes(registry.verify_download(settings, item["endpoint"], item))
        registry.verify_signatures(directory, settings, downloads, list(inventory))
    finally:
        downloads_directory.close()
    if registry.remote_tag(settings, state["tag"]) != state["sha"]:
        raise policy.ToolError("the release tag is absent or identifies another revision")
    if settings["provider"] == "github":
        current = registry.document(settings, f"releases/{state['release_id']}")
        if not isinstance(current, dict) or not current.get("draft") or current.get("tag_name") != state["tag"]:
            raise policy.ToolError("the release is no longer the staged draft; verify it manually")
        listed = {item["name"] for item in registry.pages(settings, f"releases/{state['release_id']}/assets")}
        if listed != set(inventory):
            raise policy.ToolError("draft assets differ from upload.json")
        registry.document(settings, f"releases/{state['release_id']}", "PATCH", {"draft": False})
        visible = registry.document(settings, "releases/tags/" + urllib.parse.quote(state["tag"], safe=""))
        if not isinstance(visible, dict) or visible.get("draft") or visible.get("prerelease"):
            raise policy.ToolError("published release visibility cannot be verified")
        return
    created = registry.document(
        settings,
        "releases",
        "POST",
        {
            "tag_name": state["tag"],
            "ref": state["sha"],
            "name": state["tag"],
            "description": notes(directory, state["tag"], tagged=True),
            "assets": {"links": [item["link"] for item in state["files"].values()]},
        },
    )
    if not isinstance(created, dict) or created.get("tag_name") != state["tag"] or (created.get("commit") or {}).get("id") != state["sha"]:
        raise policy.ToolError("published release tag identity cannot be verified")
    remote = registry.document(settings, "releases/" + urllib.parse.quote(state["tag"], safe="")) or {}
    if {link.get("name") for link in (remote.get("assets") or {}).get("links", [])} != set(inventory):
        raise policy.ToolError("release links differ from upload.json")


# === Private Functions === #

# === Entry Point === #
