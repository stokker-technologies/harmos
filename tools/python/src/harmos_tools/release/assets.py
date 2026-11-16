# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Assets."""

# === Imports === #

# Standard Library Imports
import hashlib
import shutil
import zipfile
from datetime import UTC, datetime
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.package.run

from ..package.changelog import notes
from . import layout, policy, reports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

SUMS = "SHA256SUMS"
STATE = "published.json"
ASSETS = "target/release-assets"
SHARED = "any"

# === Types === #

# === Classes === #

# === Public Functions === #


def inventory(directory: Path, settings: dict, selected: str, tag: str, profile: str | None = None) -> list[dict]:
    """Resolve The Platform's Reference Host Archive From Its Existing Package."""
    profile = policy.profile() if profile is None else profile
    name = layout.conformance_name(directory, settings, selected, tag[1:])
    relative = f"target/package/{profile}/{selected}/rust/{name}"
    harmos_tools.package.run.regular(directory, relative)
    return [{"name": name, "path": relative, "target": selected}]


def stage(
    directory: Path,
    settings: dict,
    tag: str,
    targets: list[str],
    profile: str | None = None,
    *,
    source_revision: str | None = None,
    release_notes: str | None = None,
    channel: str = "stable",
) -> list[dict]:
    """Replace Release Staging With The Exact Channel Inventory From Existing Packages."""
    if channel not in {"stable", "nightly"}:
        raise policy.ToolError("release channel must be stable or nightly")
    if not targets:
        raise policy.ToolError("release staging requires at least one packaged target")
    profile = policy.profile() if profile is None else profile
    root = staging_root(directory)
    if root.is_symlink() or root.exists() and not root.is_dir():
        raise policy.ToolError(f"release staging requires a regular directory: {root}")
    # Validate Directory Shapes Before Removing Any Owned Staging.
    for owned in [*targets, SHARED]:
        destination = policy.inside(root, owned)
        if destination == root or destination.is_symlink() or destination.exists() and not destination.is_dir():
            raise policy.ToolError(f"release staging requires a regular directory: {destination}")
    for owned in [*targets, SHARED]:
        destination = policy.inside(root, owned)
        if destination.exists():
            shutil.rmtree(destination)
        destination.mkdir(parents=True)
    shared = root / SHARED
    files = []
    for selected in targets:
        for asset in inventory(directory, settings, selected, tag, profile):
            destination = root / selected / asset["name"]
            shutil.copyfile(harmos_tools.package.run.regular(directory, asset["path"]), destination)
            files.append(describe(root, destination))
    content = notes(directory, tag) if release_notes is None else release_notes
    if source_revision is not None:
        content += f"\nSource Commit: {source_revision}\n"
    protocol = shared / f"harmos-proto-{tag[1:]}.zip"
    source = directory / "proto"
    definitions = sorted(source.rglob("*.proto"))
    if not definitions:
        raise policy.ToolError("protocol archive requires definitions")
    behaviour = harmos_tools.package.run.regular(directory, "proto/harmos/plugin/v1/behaviour.md")
    archive = zipfile.ZipFile(protocol, "w", compression=zipfile.ZIP_DEFLATED)
    try:
        for path in [*definitions, behaviour]:
            original = harmos_tools.package.run.regular(source, path.relative_to(source).as_posix())
            archive.write(original, original.relative_to(source).as_posix())
    finally:
        archive.close()
    files.append(describe(root, protocol))
    compliance = shared / f"harmos-compliance-{tag[1:]}.zip"
    archive = zipfile.ZipFile(compliance, "w", compression=zipfile.ZIP_DEFLATED)
    try:
        for selected in targets:
            source = directory / "target/package" / profile / selected / "compliance"
            if not source.is_dir() or source.is_symlink():
                raise policy.ToolError(f"missing regular compliance directory: {source}")
            abi = layout.triple(directory, settings, selected)
            for name in ("NOTICE", "THIRD-PARTY-NOTICES", f"crates/{abi}/workspace.cdx.json", "python/any/sdk.cdx.json"):
                original = harmos_tools.package.run.regular(source, name)
                # Preserve Each Target's Notices; Python's Locked Inventory Is Shared.
                if name.startswith("python/") and selected != targets[0]:
                    first = directory / "target/package" / profile / targets[0] / "compliance" / name
                    if original.read_bytes() != first.read_bytes():
                        raise policy.ToolError(f"Stage Python Compliance: inventories differ between {targets[0]} and {selected}: {name}")
                    continue
                member = f"notices/{selected}/{name}" if "/" not in name else name
                archive.write(original, member)
    finally:
        archive.close()
    files.append(describe(root, compliance))
    if channel == "nightly":
        for destination in reports.stage(directory, shared, tag[1:]):
            files.append(describe(root, destination))
    manifest = layout.downloads(
        settings,
        version=tag[1:],
        channel=channel,
        tag=tag if channel == "stable" else channel,
        published=datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
        files=files,
        sums="".join(f"{item['sha256']}  {item['name']}\n" for item in files),
    )
    policy.write_json(shared / "release.json", {"notes": content, "pub_date": manifest["pub_date"], "downloads": manifest})
    files.append(describe(root, shared / "release.json"))
    (shared / SUMS).write_text("".join(f"{item['sha256']}  {item['name']}\n" for item in files), encoding="utf-8")
    files.append(describe(root, shared / SUMS))
    layout.upload_manifest(directory, root, files)
    return files


def checksums(path: Path, inventory: dict) -> None:
    expected = {name: item["sha256"] for name, item in inventory.items() if name != SUMS}
    actual = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        digest, separator, name = line.partition("  ")
        if not separator or name in actual:
            raise policy.ToolError("invalid or duplicate SHA256SUMS entry")
        actual[name] = digest
    if actual != expected:
        raise policy.ToolError("SHA256SUMS differs from upload.json")


def read(root: Path, item: dict) -> bytes:
    data = policy.inside(root, item["path"]).read_bytes()
    if hashlib.sha256(data).hexdigest() != item["sha256"]:
        raise policy.ToolError(f"staged release bytes changed: {item['name']}")
    return data


def describe(root: Path, path: Path) -> dict:
    data = path.read_bytes()
    return {"name": path.name, "path": path.relative_to(root).as_posix(), "sha256": hashlib.sha256(data).hexdigest(), "size": len(data)}


def staging_root(directory: Path) -> Path:
    return policy.inside(directory, ASSETS)


def package(settings: dict) -> str:
    """The GitLab Generic Package That Carries The Release Downloads."""
    return settings.get("package", settings["product"])


# === Private Functions === #

# === Entry Point === #
