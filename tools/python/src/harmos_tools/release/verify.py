# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Verify."""

# === Imports === #

# Standard Library Imports
import urllib.parse
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.release.assets
import harmos_tools.release.conformance
import harmos_tools.release.source

from .. import temporary
from . import layout, policy, registry

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(directory: Path, settings: dict) -> None:
    """After Publication: Download Every Published Asset, Match SHA256SUMS And Signatures, Install On This Platform."""
    tag = harmos_tools.release.source.release_tag(directory)
    sha = harmos_tools.release.source.revision(directory)
    if registry.remote_tag(settings, tag) != sha:
        raise policy.ToolError(f"{tag} does not identify the checked-out revision")
    inventory = {item["name"]: item for item in layout.uploads(directory)}
    quoted = urllib.parse.quote(tag, safe="")
    if settings["provider"] == "github":
        remote = registry.document(settings, "releases/tags/" + quoted)
        if not isinstance(remote, dict) or remote.get("draft") or remote.get("prerelease"):
            raise policy.ToolError("release is not published")
        downloads = {item["name"]: f"releases/assets/{item['id']}" for item in registry.pages(settings, f"releases/{remote['id']}/assets")}
    else:
        remote = registry.document(settings, "releases/" + quoted)
        if not isinstance(remote, dict):
            raise policy.ToolError("release is not published")
        prefix = f"packages/generic/{urllib.parse.quote(harmos_tools.release.assets.package(settings), safe='')}/{quoted}/"
        downloads = {item["name"]: prefix + urllib.parse.quote(item["name"], safe="") for item in remote["assets"]["links"]}
    if set(downloads) != set(inventory):
        raise policy.ToolError("published downloads differ from upload.json")
    root_directory = temporary.Directory()
    root = root_directory.path
    try:
        for name, item in inventory.items():
            data = registry.verify_download(settings, downloads[name], item)
            policy.inside(root, name).write_bytes(data)
        harmos_tools.release.assets.checksums(root / harmos_tools.release.assets.SUMS, inventory)
        registry.verify_signatures(directory, settings, root, list(inventory))
        native = harmos_tools.release.source.native_target(settings)
        harmos_tools.release.conformance.verify(directory, settings, "release", native, root)
    finally:
        root_directory.close()
    policy.write_json(directory / "target/state/release-publication.json", {"sha": sha, "tag": tag, "verified": True})


# === Private Functions === #

# === Entry Point === #
