# Rendered by stkr project update from templates/python/release/screenshots.py; sha256:63c307c75855482f06929a187915edefa02943d54d629dab9468515f1d97fbcc
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Capture And Validate Flat Screenshots With Revision Metadata"""

# === Imports === #

# Standard Library Imports
import json
import re
import struct
from datetime import UTC, datetime
from pathlib import Path

# Third Party Imports
# Local Imports
# isort: split
from ..test import archive
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

PNG = b"\x89PNG\r\n\x1a\n"
MAX_BYTES = 4 * 1024 * 1024

# === Types === #

# === Classes === #

# === Public Functions === #


def dimensions(data: bytes) -> tuple[int, int]:
    if len(data) < 33 or data[:8] != PNG or data[12:16] != b"IHDR":
        raise policy.ToolError("screenshot must contain a PNG IHDR")
    return struct.unpack(">II", data[16:24])


def check(directory: Path, _: str = "check") -> None:
    settings = policy.policy(directory, "screenshots")
    folder = policy.inside(directory, "screenshots")
    if not folder.is_dir():
        raise policy.ToolError("screenshots/ is missing")
    readme = (folder / "README.md").read_text(encoding="utf-8")
    manifest = json.loads((folder / "manifest.json").read_text(encoding="utf-8"))
    if not isinstance(manifest, list):
        raise policy.ToolError("screenshot manifest must be an array")
    entries = {}
    for entry in manifest:
        if not isinstance(entry, dict) or not all(
            key in entry
            for key in [
                "image",
                "surface",
                "state",
                "viewport",
                "revision",
                "captured_at",
            ]
        ):
            raise policy.ToolError("screenshot manifest entry is incomplete")
        image = entry["image"]
        if image in entries or image != f"{entry['surface']}-{entry['state']}.png":
            raise policy.ToolError(f"duplicate or inconsistent screenshot: {image}")
        if not re.fullmatch(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)+\.png", image):
            raise policy.ToolError(f"invalid screenshot name: {image}")
        entries[image] = entry
    images = set()
    for path in folder.iterdir():
        if path.is_symlink() or not path.is_file():
            raise policy.ToolError(f"screenshots must be flat regular files: {path.name}")
        if path.name in ["README.md", "manifest.json"]:
            continue
        if path.suffix != ".png" or path.stat().st_size > MAX_BYTES:
            raise policy.ToolError(f"screenshot must be PNG at most 4 MiB: {path.name}")
        images.add(path.name)
        if path.name not in entries or not re.search(r"\]\((?:\./)?" + re.escape(path.name) + r"\)", readme):
            raise policy.ToolError(f"screenshot missing manifest or README reference: {path.name}")
        entry = entries[path.name]
        width, height = dimensions(path.read_bytes())
        if entry["viewport"] != {"width": width, "height": height}:
            raise policy.ToolError(f"screenshot viewport mismatch: {path.name}")
        timestamp = datetime.fromisoformat(entry["captured_at"])
        if timestamp.tzinfo is None:
            raise policy.ToolError("captured_at requires a timezone")
        revision = entry["revision"]
        if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40,64}", revision):
            raise policy.ToolError("revision requires a full commit hash")
        branch = settings.get("integration_branch", "main")
        if not isinstance(branch, str) or re.fullmatch(r"[A-Za-z0-9/._-]+", branch) is None:
            raise policy.ToolError("invalid integration branch name")
        policy.execute(
            directory,
            ["git", "merge-base", "--is-ancestor", revision, branch],
            capture=True,
        )
        behind = int(
            policy.execute(
                directory,
                ["git", "rev-list", "--count", f"{revision}..{branch}"],
                capture=True,
            )
        )
        limit = settings.get("max_commits_behind", 0)
        if not isinstance(limit, int) or limit < 0 or behind > limit:
            raise policy.ToolError(f"screenshot revision is {behind} commits behind {branch}: {path.name}")
    if images != set(entries):
        raise policy.ToolError("screenshot manifest contains missing images")


def capture(directory: Path, _: str = "capture") -> None:
    settings = policy.policy(directory, "screenshots")
    scenes = settings.get("scenes", [])
    if not isinstance(scenes, list) or not scenes:
        raise policy.ToolError("screenshots.scenes must declare capture commands")
    folder = policy.inside(directory, "screenshots")
    folder.mkdir(exist_ok=True)
    revision = archive.revision(directory)
    if revision is None:
        raise policy.ToolError("cannot determine screenshot revision")
    entries = []
    for scene in scenes:
        surface, state = scene["surface"], scene["state"]
        image = f"{surface}-{state}.png"
        if not re.fullmatch(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)+\.png", image):
            raise policy.ToolError(f"invalid scene name: {image}")
        target = policy.inside(folder, image)
        target.unlink(missing_ok=True)
        arguments = policy.command(scene.get("command"), "screenshots.scenes.command")
        policy.execute(directory, [part.replace("{output}", str(target)) for part in arguments])
        width, height = dimensions(target.read_bytes())
        entries.append(
            {
                "image": image,
                "surface": surface,
                "state": state,
                "viewport": {"width": width, "height": height},
                "revision": revision,
                "captured_at": datetime.now(UTC).isoformat(),
            }
        )
    policy.write_json(folder / "manifest.json", entries)
    check(directory)


def run(directory: Path, verb: str) -> None:
    if verb == "capture":
        capture(directory)
    else:
        check(directory)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(policy.main("screenshots", ["capture", "check"], run))
