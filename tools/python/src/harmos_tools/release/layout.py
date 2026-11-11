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
import hashlib
import json
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
    value = settings.get("target_triples", {}).get(target)
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_]+(?:-[A-Za-z0-9_]+){2,}", value):
        raise policy.ToolError(f"declare release.target_triples for {target}")
    return value


def conformance_name(directory: Path, settings: dict, target: str, version: str) -> str:
    abi = triple(directory, settings, target)
    suffix = ".zip" if "-windows-" in abi else ".tar.gz"
    return f"{settings['product']}-conformance-{version}-{abi}{suffix}"


def download_url(settings: dict, tag: str, name: str) -> str:
    """The Public Download Of One Release Asset Under Its Tag, From The Declared Provider."""
    quoted = f"{urllib.parse.quote(tag, safe='')}/{urllib.parse.quote(name, safe='')}"
    if settings["provider"] == "github":
        return f"https://github.com/{settings['repository']}/releases/download/{quoted}"
    base = settings.get("api_url", "https://gitlab.com/api/v4").removesuffix("/api/v4")
    return f"{base}/{settings['project_id']}/-/releases/{quoted.replace('/', '/downloads/', 1)}"


def classify(settings: dict, name: str) -> dict:
    for family in ("proto", "compliance", "reports"):
        if name.startswith(f"{settings['product']}-{family}-") and name.endswith(".zip"):
            return {"kind": family, "os": "any", "arch": "any", "format": "zip"}
    if name == "benchmarks.json":
        return {"kind": "benchmarks", "os": "any", "arch": "any", "format": "json"}
    match = re.fullmatch(r"harmos-conformance-.+-(x86_64|aarch64)-[a-z0-9_]+-(linux|windows|darwin).*(\.zip|\.tar\.gz)", name)
    if match:
        return {"kind": "conformance", "os": {"darwin": "macos"}.get(match[2], match[2]), "arch": match[1], "format": match[3][1:]}
    raise policy.ToolError(f"unsupported release download: {name}")


def downloads(settings: dict, *, version: str, channel: str, tag: str, published: str, files: list[dict], sums: str) -> dict:
    """Classify And Link Payloads; The Index And Checksums Stay Outside Their Own Inventory."""
    digests = {}
    for line in sums.splitlines():
        digest, separator, name = line.partition("  ")
        if not separator or name in digests:
            raise policy.ToolError("invalid or duplicate SHA256SUMS entry")
        digests[name] = digest
    artifacts = []
    for item in sorted(files, key=lambda entry: entry["name"]):
        digest = digests.get(item["name"])
        if digest is None:
            raise policy.ToolError(f"SHA256SUMS does not list {item['name']}")
        artifacts.append(
            {
                **classify(settings, item["name"]),
                "name": item["name"],
                "url": download_url(settings, tag, item["name"]),
                "size": item["size"],
                "sha256": digest,
            }
        )
    return {"version": version, "channel": channel, "tag": tag, "pub_date": published, "artifacts": artifacts}


def upload_manifest(directory: Path, root: Path, files: list[dict]) -> None:
    entries = [{**item, "path": (root / item["path"]).relative_to(directory).as_posix()} for item in files]
    policy.write_json(root / "upload.json", {"format_version": 1, "assets": entries})


def uploads(directory: Path, *, staged: bool = False) -> list[dict]:
    """Read The Exact Publication Inventory, Refusing Unsafe Names, Paths And Changed Bytes."""
    path = policy.inside(directory, "target/release-assets/upload.json")
    if not path.is_file() or path.is_symlink():
        raise policy.ToolError("release upload.json manifest is missing or linked")
    manifest = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(manifest, dict) or manifest.get("format_version") != 1 or not isinstance(manifest.get("assets"), list):
        raise policy.ToolError("unsupported release upload manifest")
    names = set()
    for item in manifest["assets"]:
        if not isinstance(item, dict) or set(item) != {"name", "path", "sha256", "size"}:
            raise policy.ToolError("invalid release upload entry")
        name, relative, digest, size = (item[key] for key in ("name", "path", "sha256", "size"))
        if not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9_.-]+", name) or name in {".", "..", "upload.json", "published.json"}:
            raise policy.ToolError("unsafe release upload name")
        if name.casefold() in names:
            raise policy.ToolError("release upload names collide")
        names.add(name.casefold())
        if not isinstance(relative, str) or not re.fullmatch(r"target/release-assets/[A-Za-z0-9_-]+/" + re.escape(name), relative):
            raise policy.ToolError("unsafe release upload path")
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest) or type(size) is not int or size < 0:
            raise policy.ToolError("invalid release upload digest or size")
        if staged:
            source = policy.inside(directory, relative)
            if not source.is_file() or source.is_symlink():
                raise policy.ToolError(f"missing staged upload: {name}")
            data = source.read_bytes()
            if len(data) != size or hashlib.sha256(data).hexdigest() != digest:
                raise policy.ToolError(f"staged upload changed: {name}")
    if "sha256sums" not in names:
        raise policy.ToolError("release upload manifest has no SHA256SUMS")
    return manifest["assets"]


# === Private Functions === #

# === Entry Point === #
