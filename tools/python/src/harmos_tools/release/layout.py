# Rendered by stkr project update from templates/python/release/layout.py; sha256:ab902403c65512ebc3273f7aef220158797770f94fe7ccf72336072eb46b1c7a
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Resolve Product-Owned Release Downloads"""

# === Imports === #

# Standard Library Imports
import re
import urllib.parse
from pathlib import Path

# Third Party Imports

# Local Imports
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def triple(directory: Path, settings: dict, target: str) -> str:
    """Resolve A Product's Actual ABI From Its Declarations, Never From Hub Product Names."""
    candidates = {
        item["targets"][target] for item in policy.config(directory).get("package", {}).get("executables", []) if target in item["targets"]
    }
    candidates.update(item["targets"][target] for item in settings.get("plugins", []) if target in item["targets"])
    declared = settings.get("target_triples", {}).get(target)
    if declared:
        candidates.add(declared)
    if len(candidates) != 1:
        raise policy.ToolError(f"declare one release.target_triples entry or executable ABI for {target}")
    value = candidates.pop()
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_]+(?:-[A-Za-z0-9_]+){2,}", value):
        raise policy.ToolError("invalid release Rust target triple")
    return value


def asset_name(directory: Path, settings: dict, definition: dict, selected: str, tag: str, source: Path) -> str | None:
    """Accept Only The Locked Download Kinds; Product Examples And Registry Artifacts Stay Out."""
    if source.name == "latest.json":
        return "latest.json"
    parts = source.relative_to(directory).parts
    if source.suffix.lower() in {".sig", ".whl", ".dokime"} or "python" in parts or "examples" in parts:
        return None
    kind = definition.get("kind", "installer")
    name = definition.get("name", source.name)
    substitutions = {"product": settings["product"], "version": tag[1:], "target": selected}
    if kind == "payload":
        if source.suffix != ".zip":
            raise policy.ToolError("distributor payload must be a ZIP")
        return f"{settings['product']}-payload-{tag[1:]}-{triple(directory, settings, selected)}.zip"
    name = name.format(**substitutions)
    if kind == "tool":
        return name
    if source.suffix.lower() in {".deb", ".rpm"}:
        return name
    if source.suffix.lower() not in {".exe", ".msi", ".msix", ".appimage", ".dmg", ".pkg"}:
        raise policy.ToolError(f"unsupported release asset; declare payloads with kind = 'payload': {source.name}")
    suffix = "-setup.exe" if source.name.endswith("-setup.exe") else source.suffix
    return f"{settings['product']}-{tag[1:]}-{triple(directory, settings, selected)}{suffix}"


def plugins(directory: Path, settings: dict, tag: str, targets: list[str], profile: str) -> list[dict]:
    """One Explicitly Shipped Package Per Feed v1, With One Native Bundle Per Target."""
    declarations = settings.get("plugins", [])
    if len(declarations) > 1:
        raise policy.ToolError("plugins.json format_version 1 supports exactly one package; extend the reader first")
    result = []
    for declaration in declarations:
        package = declaration["package"]
        if not isinstance(package, str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,128}", package):
            raise policy.ToolError("invalid shipped plugin package")
        requirement = declaration["requires_dokime"]
        if not isinstance(requirement, str) or not requirement.strip() or len(requirement) > 256:
            raise policy.ToolError("plugin requires_dokime must declare its host requirement")
        for target in targets:
            abi = declaration["targets"].get(target)
            if abi is None:
                continue
            if abi != triple(directory, settings, target):
                raise policy.ToolError("plugin target differs from the declared release ABI")
            relative = declaration["path"].format(profile=profile, target=target, version=tag[1:], package=package)
            source = policy.inside(directory, relative)
            if source.suffix != ".dokime" or not source.is_file() or source.is_symlink():
                raise policy.ToolError(f"missing regular shipped plugin bundle: {relative}")
            result.append({"name": f"{package}-{tag[1:]}-{abi}.dokime", "path": relative, "target": target, "triple": abi})
    if len(result) > 32:
        raise policy.ToolError("plugins.json supports at most 32 targets")
    return result


def plugin_feed(settings: dict, tag: str, bundles: list[dict]) -> dict:
    declaration = settings["plugins"][0]
    entries = []
    for bundle in bundles:
        name = urllib.parse.quote(bundle["name"], safe="")
        if settings["provider"] == "github":
            url = f"https://github.com/{settings['repository']}/releases/download/{tag}/{name}"
        else:
            base = settings.get("api_url", "https://gitlab.com/api/v4").rstrip("/")
            project = urllib.parse.quote(str(settings["project_id"]), safe="")
            package = urllib.parse.quote(settings.get("package", settings["product"]), safe="")
            url = f"{base}/projects/{project}/packages/generic/{package}/{tag}/{name}"
        if urllib.parse.urlsplit(url).scheme != "https":
            raise policy.ToolError("plugin release URLs must use HTTPS")
        entries.append({"target": bundle["triple"], "url": url, "sha256": bundle["sha256"]})
    return {
        "format_version": 1,
        "package": declaration["package"],
        "version": tag[1:],
        "requires_dokime": declaration["requires_dokime"],
        "assets": entries,
    }


def upload_manifest(directory: Path, root: Path, files: list[dict]) -> None:
    entries = [{**item, "path": (root / item["path"]).relative_to(directory).as_posix()} for item in files]
    policy.write_json(root / "upload.json", {"format_version": 1, "assets": entries})


# === Private Functions === #

# === Entry Point === #
