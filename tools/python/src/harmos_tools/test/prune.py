# Rendered by stkr project update from templates/python/test/prune.py; sha256:a53159b34ecc580c04face9e06312f9d17adcb83d4ba1bf1efddc3ab334c1def
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Prune GitLab Compiler Caches To Recently Used Dependency Artifacts."""

# === Imports === #

# Standard Library Imports
import argparse
import json
import shutil
import subprocess
import time
from pathlib import Path

# Third Party Imports

# Local Imports
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

STAMP = Path(".cache/compiler-start.json")

# === Types === #

# === Classes === #

# === Public Functions === #


def prune(directory: Path) -> None:
    state = json.loads((directory / STAMP).read_text())
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=directory))
    members = set(metadata["workspace_members"])
    names = {
        name.replace("-", "_")
        for package in metadata["packages"]
        if package["id"] in members
        for name in [package["name"], *(target["name"] for target in package["targets"])]
    }
    target = directory / "target"
    for profile in ("debug", "release"):
        base = target / profile
        if not base.is_dir():
            continue
        for entry in base.iterdir():
            if entry.name not in ("deps", "build", ".fingerprint"):
                _remove(entry)
                continue
            for artifact in entry.iterdir():
                name = artifact.name.removeprefix("lib").replace("-", "_")
                files = list(artifact.rglob("*")) if artifact.is_dir() else [artifact]
                touched = any(max(file.stat().st_atime, file.stat().st_mtime) >= state["started"] for file in files if file.is_file())
                workspace = any(name == member or name.startswith((member + "_", member + ".")) for member in names)
                if workspace or not touched or artifact.is_symlink():
                    _remove(artifact)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("operation", choices=("start", "prune"))
    options = parser.parse_args()
    directory = policy.root()
    if options.operation == "start":
        path = directory / STAMP
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"started": time.time()}))
    else:
        prune(directory)


# === Private Functions === #


def _remove(path: Path) -> None:
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path)
    else:
        path.unlink(missing_ok=True)


# === Entry Point === #

if __name__ == "__main__":
    main()
