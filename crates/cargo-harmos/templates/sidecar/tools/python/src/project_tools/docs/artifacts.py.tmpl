# Rendered by stkr project update from templates/python/docs/artifacts.py; sha256:b2f5b333ccf58de3b3eaaca53d3acd7c594d39e351e18e7b6eee4ffc4104b588
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Fingerprint, Verify, And Assemble Immutable Documentation Artifacts"""

# === Imports === #

# Standard Library Imports
import hashlib
import io
import json
import re
import shutil
import zipfile
from pathlib import Path

# Third Party Imports
# Local Imports
from ..release import policy
from . import site

# Type Checking Imports

# === Public Exports === #

# === Constants === #

SEMVER = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z.-]+))?(?:\+([0-9A-Za-z.-]+))?")
NAVIGATION = """document.querySelectorAll('[data-docs-navigation]').forEach(async (host) => {
  try {
    const metadata = host.dataset.metadata;
    const catalog = await fetch(metadata, {cache: 'no-cache'}).then((response) => {
      if (!response.ok) throw new Error('Version Catalog');
      return response.json();
    });
    const base = new URL(metadata, location.href).pathname.replace(/versions.json$/, '');
    const relative = location.pathname.slice(base.length);
    const selected = relative.split('/')[0];
    const entry = catalog.versions.find((version) => version.selector === selected);
    const select = document.createElement('select');
    select.setAttribute('aria-label', 'Documentation Version');
    for (const version of catalog.versions) {
      const option = document.createElement('option');
      option.value = version.selector; option.textContent = version.selector;
      option.selected = option.value === selected; select.append(option);
    }
    select.addEventListener('change', () => {
      const version = catalog.versions.find((item) => item.selector === select.value);
      const path = relative.split('/').slice(1).join('/');
      const file = path.endsWith('/') || !path ? path + 'index.html' : path;
      const target = version.pages.includes(file) ? path : version.home;
      location.href = base + version.selector + '/' + target;
    });
    host.append('Version', select);
    if (entry && selected !== catalog.latest && selected !== 'development') {
      const banner = document.createElement('p'); banner.dataset.docsArchive = '';
      banner.textContent = 'Archived Version. See the latest stable documentation for current behavior.';
      host.append(banner);
    }
    if (catalog.roadmap) {
      const link = document.createElement('a'); link.href = catalog.roadmap;
      link.textContent = 'Roadmap'; host.append(link);
    }
  } catch (error) { host.textContent = 'Version Navigation Unavailable'; }
});
"""

# === Types === #

# === Classes === #

# === Public Functions === #


def semver(value: str) -> tuple[int, int, int, str]:
    match = SEMVER.fullmatch(value)
    if match and match[5] and any(not item for item in match[5].split(".")):
        raise policy.ToolError(f"invalid semantic version: {value}")
    if not match or (
        match[4] and any(not item or (item.isdigit() and len(item) > 1 and item.startswith("0")) for item in match[4].split("."))
    ):
        raise policy.ToolError(f"invalid semantic version: {value}")
    return (int(match[1]), int(match[2]), int(match[3]), match[4] or "")


def eligible(settings: dict, value: str) -> bool:
    major, _, _, pre = semver(value)
    for key in ["allow_unstable", "allow_prerelease", "guides", "api", "development"]:
        if key in settings and not isinstance(settings[key], bool):
            raise policy.ToolError(f"docs.{key} must be a boolean")
    return (major != 0 or settings.get("allow_unstable", False)) and (not pre or settings.get("allow_prerelease", True))


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def inventory(folder: Path) -> dict[str, str]:
    files = {}
    for path in sorted(folder.rglob("*")):
        if path.is_symlink():
            raise policy.ToolError(f"linked documentation artifact: {path}")
        if path.is_file():
            files[path.relative_to(folder).as_posix()] = digest(path.read_bytes())
    if not files:
        raise policy.ToolError("empty documentation artifact")
    return files


def verify(folder: Path, manifest: dict) -> None:
    if manifest.get("schema") != 1 or manifest.get("files") != inventory(folder):
        raise policy.ToolError("documentation artifact manifest or content differs")
    selector = manifest.get("selector", "")
    if selector != "development" and not selector.startswith("v"):
        raise policy.ToolError("invalid artifact selector")
    if selector != "development" and selector != "v" + manifest["version"]:
        raise policy.ToolError("release selector differs from artifact version")
    semver(manifest["version"])
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", manifest.get("revision", "")):
        raise policy.ToolError("artifact has no source revision")
    if not manifest.get("inputs") or not manifest.get("tools"):
        raise policy.ToolError("artifact lacks input/tool fingerprints")
    pages = site.static(folder, manifest["base"] + selector + "/", shared=True)
    if pages != manifest.get("pages") or manifest.get("home") not in pages:
        raise policy.ToolError("artifact navigation differs from manifest")


def archive(folder: Path, manifest: dict) -> bytes:
    verify(folder, manifest)
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as stream:
        stream.writestr("manifest.json", json.dumps(manifest, sort_keys=True))
        for name in sorted(manifest["files"]):
            stream.writestr("files/" + name, (folder / name).read_bytes())
    return output.getvalue()


def unpack(data: bytes, destination: Path) -> dict:
    if destination.exists():
        raise policy.ToolError("artifact destination must be clean")
    destination.mkdir(parents=True)
    with zipfile.ZipFile(io.BytesIO(data)) as stream:
        names = stream.namelist()
        if len(names) != len(set(names)) or "manifest.json" not in names:
            raise policy.ToolError("invalid documentation archive entries")
        manifest = json.loads(stream.read("manifest.json"))
        expected = {"manifest.json", *("files/" + name for name in manifest["files"])}
        if set(names) != expected:
            raise policy.ToolError("archive entries differ from manifest")
        for name in names:
            if name == "manifest.json":
                continue
            target = policy.inside(destination, name.removeprefix("files/"))
            if ".." in Path(name).parts or Path(name).is_absolute():
                raise policy.ToolError("unsafe documentation archive path")
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(stream.read(name))
    verify(destination, manifest)
    return manifest


def assemble(
    destination: Path,
    artifacts: list[tuple[Path, dict]],
    base: str,
    roadmap: str | None,
) -> dict:
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    published = policy.inside(destination, base.strip("/"))
    published.mkdir(parents=True, exist_ok=True)
    catalog = []
    for folder, manifest in artifacts:
        verify(folder, manifest)
        selector = manifest["selector"]
        if manifest["base"] != base or any(item["selector"] == selector for item in catalog):
            raise policy.ToolError("conflicting artifact selector or base")
        shutil.copytree(folder, policy.inside(published, selector))
        catalog.append({key: manifest[key] for key in ["selector", "version", "revision", "pages", "home"]})
    stable = [
        item for item in catalog if item["selector"] != "development" and semver(item["version"])[0] > 0 and not semver(item["version"])[3]
    ]
    latest = max(stable, key=lambda item: semver(item["version"])[:3])["selector"] if stable else None
    catalog.sort(
        key=lambda item: (
            item["selector"] == "development",
            semver(item["version"])[:3],
            item["version"],
        ),
        reverse=True,
    )
    metadata = {"schema": 1, "latest": latest, "versions": catalog, "roadmap": roadmap}
    policy.write_json(published / "versions.json", metadata)
    (published / "versions.js").write_text(NAVIGATION, encoding="utf-8")
    target = next(item for item in catalog if item["selector"] == (latest or catalog[0]["selector"]))
    redirect = f"{base}{target['selector']}/{target['home']}"
    (published / "index.html").write_text(
        f'<html><head><meta http-equiv="refresh" content="0;url={redirect}"></head><body><a href="{redirect}">Documentation</a></body></html>',
        encoding="utf-8",
    )
    # Latest Is A Serving Alias; Immutable Version Content Is Never Rewritten.
    if latest:
        shutil.copytree(published / latest, published / "latest")
        for path in (published / "latest").rglob("*.html"):
            suffix = path.relative_to(published / "latest").as_posix()
            url = base + latest + "/" + suffix
            path.write_text(
                f'<html><head><meta http-equiv="refresh" content="0;url={url}"></head><body><a href="{url}">Latest Stable Documentation</a></body></html>',
                encoding="utf-8",
            )
    site.static(destination, "/")
    return metadata


# === Private Functions === #

# === Entry Point === #
