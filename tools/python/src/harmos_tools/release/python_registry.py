# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Publish Python Distributions."""

# === Imports === #

# Standard Library Imports
import hashlib
import json
import os
import tomllib
import urllib.error
import urllib.parse
import urllib.request
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


def publish(directory: Path, settings: dict, *, dry_run: bool) -> None:
    """Upload The Release Python Packages: PyPI For GitHub Products, The Project Registry On GitLab."""
    profile = policy.profile()
    selected = sorted(str(path.relative_to(directory)) for path in directory.glob(f"target/build/{profile}/any/python/*") if path.is_file())
    if not selected:
        raise policy.ToolError(f"release:publish:python has no {profile} Python packages")
    if dry_run:
        # Build and check only: uv validates every file without credentials or upload.
        credentials = policy.CredentialScope(settings, profile=profile)
        try:
            policy.execute(directory, ["uv", "publish", "--dry-run", "--trusted-publishing", "never", *selected])
        finally:
            credentials.close()
        return
    if settings["provider"] == "github":
        project = tomllib.loads((directory / "sdk/python/pyproject.toml").read_text(encoding="utf-8"))["project"]["name"]
        try:
            response = urllib.request.urlopen(f"https://pypi.org/pypi/{urllib.parse.quote(project, safe='')}/json", timeout=60)
            try:
                releases = json.load(response)["releases"]
            finally:
                response.close()
        except urllib.error.HTTPError as error:
            error.close()
            if error.code != 404:
                raise
            releases = {}
        published = {item["filename"]: item["digests"]["sha256"] for files in releases.values() for item in files}
        missing = []
        for relative in selected:
            path = directory / relative
            if path.name not in published:
                missing.append(relative)
                continue
            if hashlib.sha256(path.read_bytes()).hexdigest() != published[path.name]:
                raise policy.ToolError(f"PyPI file already exists with a different SHA-256: {path.name}")
            print(f"PyPI file already published with matching SHA-256: {path.name}")
        if missing:
            policy.execute(
                directory, ["uv", "publish", "--trusted-publishing", "always", "--check-url", "https://pypi.org/simple/", *missing]
            )
        return
    base = os.environ.get("CI_API_V4_URL", settings.get("api_url", "https://gitlab.com/api/v4"))
    project = settings.get("project_id", os.environ.get("CI_PROJECT_ID"))
    token = os.environ.get("CI_JOB_TOKEN")
    if not token or not project:
        raise policy.ToolError("GitLab Python publication requires the native CI_JOB_TOKEN")
    destination = f"{base}/projects/{urllib.parse.quote(str(project), safe='')}/packages/pypi"
    command = ["uv", "publish", "--publish-url", destination, "--username", "gitlab-ci-token", "--password", token, *selected]
    policy.execute(directory, command)


# === Private Functions === #

# === Entry Point === #
