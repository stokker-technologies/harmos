# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Assemble the Published Documentation Archives into One Site Image"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import re
import shutil
import sys
from html import escape
from pathlib import Path
from zipfile import BadZipFile, ZipFile

# isort: split

# Third Party Imports

# isort: split

# Local Imports
from ..release import layout, notices, policy, registry
from . import site

# Type Checking Imports

# === Public Exports === #

# === Constants === #

OUTPUT = "target/docs/image"
DOWNLOADS = "target/docs/archives"
IMAGE = "harmos-docs-site.zip"
LATEST = "latest"
NIGHTLY = "nightly"
STABLE = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")

# === Types === #

# === Classes === #

# === Public Functions === #


def download(directory: Path) -> list[Path]:
    """Fetch the Newest Archive of Each Stable Release Line and the Nightly Archive."""
    settings = policy.policy(directory, "release")
    selected: dict[str, tuple[tuple[int, ...], str, str]] = {}
    for release in registry.pages(settings, "releases"):
        names = {asset.get("name") for asset in release.get("assets", [])}
        tag = release.get("tag_name", "")
        if release.get("draft"):
            continue
        if tag == NIGHTLY and f"harmos-docs-{NIGHTLY}.zip" in names:
            selected[site.DEVELOPMENT] = ((), tag, f"harmos-docs-{NIGHTLY}.zip")
            continue
        matched = STABLE.fullmatch(tag.removeprefix("v"))
        name = f"harmos-docs-{tag.removeprefix('v')}.zip"
        if release.get("prerelease") or not tag.startswith("v") or matched is None or name not in names:
            continue
        version = tuple(int(part) for part in matched.groups())
        line = f"{version[0]}.{version[1]}"
        if line not in selected or selected[line][0] < version:
            selected[line] = (version, tag, name)
    destination = policy.inside(directory, DOWNLOADS)
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    archives = []
    for _, tag, name in selected.values():
        target = destination / name
        target.write_bytes(notices.download(layout.download_url(settings, tag, name), operation="Download Documentation Archive"))
        archives.append(target)
    return archives


def assemble(archives: list[Path], output: Path) -> list[dict]:
    """Unpack Each Archive Under Its Entry, Then Write the Selector Entries and the latest Redirects."""
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    releases: dict[str, tuple[int, ...]] = {}
    development = None
    for archive in archives:
        source = ZipFile(archive)
        try:
            stamp = json.loads(source.read(site.STAMP))
            entry, version = stamp["entry"], stamp["version"]
            matched = STABLE.fullmatch(version)
            if entry == site.DEVELOPMENT:
                if development is not None:
                    raise policy.ToolError("Assemble Documentation: More Than One Development Archive")
                development = version
            elif matched is None or entry != f"{matched[1]}.{matched[2]}":
                raise policy.ToolError(f"Assemble Documentation: {archive.name} Is Neither a Release Line Nor Development")
            elif entry in releases:
                raise policy.ToolError(f"Assemble Documentation: More Than One Archive for Release Line {entry}")
            else:
                releases[entry] = tuple(int(part) for part in matched.groups())
            _extract(source, output / entry)
        finally:
            source.close()
    ordered = sorted(releases, key=lambda line: releases[line], reverse=True)
    entries = []
    if ordered:
        newest = ordered[0]
        entries.append({"entry": LATEST, "label": f"Latest ({_version(releases[newest])})"})
        _redirects(output / newest, output / LATEST, f"/{newest}/")
    entries += [{"entry": line, "label": _version(releases[line])} for line in ordered]
    if development is not None:
        entries.append({"entry": site.DEVELOPMENT, "label": "Development"})
    if not entries:
        raise policy.ToolError("Assemble Documentation: No Archive Was Provided")
    landing = f"/{LATEST if ordered else site.DEVELOPMENT}/"
    (output / "index.html").write_text(_redirect(landing), encoding="utf-8", newline="\n")
    policy.write_json(output / "versions.json", entries)
    return entries


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Assemble the Documentation Site Image")
    parser.add_argument("--archive", action="append", type=Path, default=[], help="Assemble These Archives Instead of Downloading")
    options = parser.parse_args(arguments)
    directory = policy.root()
    try:
        archives = [directory / path for path in options.archive] or download(directory)
        output = policy.inside(directory, OUTPUT)
        for entry in assemble(archives, output):
            print(f"{entry['entry']}: {entry['label']}")
        site.pack(output, policy.inside(directory, f"{site.OUTPUT}/{IMAGE}"))
    except (policy.ToolError, OSError, ValueError, KeyError, BadZipFile) as error:
        print(f"Documentation Image: {error}", file=sys.stderr)
        return 1
    return 0


# === Private Functions === #


def _extract(source: ZipFile, destination: Path) -> None:
    for member in source.infolist():
        target = (destination / member.filename).resolve()
        if not target.is_relative_to(destination.resolve()) or member.filename.startswith("/") or "\\" in member.filename:
            raise policy.ToolError(f"Assemble Documentation: Unsafe Archive Member {member.filename}")
    source.extractall(destination)


def _redirects(entry: Path, latest: Path, target: str) -> None:
    """Point Every Page of the Newest Release Line Under latest/ at the Same Page of That Line."""
    for page in sorted(entry.rglob("*.html")):
        relative = page.relative_to(entry).as_posix()
        location = target + relative.removesuffix("index.html")
        destination = latest / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(_redirect(location), encoding="utf-8", newline="\n")


def _redirect(location: str) -> str:
    escaped = escape(location, quote=True)
    return (
        "<!doctype html>\n"
        '<html lang="en"><head><meta charset="utf-8">'
        f'<meta http-equiv="refresh" content="0; url={escaped}">'
        f'<link rel="canonical" href="{escaped}"><title>Harmos Documentation</title>'
        f"<script>location.replace({json.dumps(location)} + location.hash)</script>"
        f'</head><body><a href="{escaped}">Harmos Documentation</a></body></html>\n'
    )


def _version(value: tuple[int, ...]) -> str:
    return ".".join(str(part) for part in value)


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
