# Rendered by stkr project update from templates/python/release/bootstrap.py; sha256:95662908e9223b91263c7b3047f8b20b77b871ea5852170691502cc1ab8f1570
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Acquire Exact Internal Releases With Verified Platform Checksums"""

# === Imports === #

# Standard Library Imports
import hashlib
import os
import platform
import re
import subprocess
import tempfile
import time
import urllib.parse
import urllib.request
from pathlib import Path

# Third Party Imports
# Local Imports
# isort: split
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ReleaseRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        origin = urllib.parse.urlsplit(req.full_url)
        target = urllib.parse.urlsplit(newurl)
        if (target.scheme, target.hostname, target.port) != (
            origin.scheme,
            origin.hostname,
            origin.port,
        ):
            raise policy.ToolError("stkr release redirect changed origin; credentials were not forwarded")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


# === Public Functions === #


def release_pin(document: dict) -> dict | None:
    pin = document.get("ci", {}).get("stkr")
    if pin is None:
        return None
    if document.get("project", {}).get("visibility") != "internal":
        raise policy.ToolError("ci.stkr requires explicit project.visibility = 'internal'; public or unclassified CI remains independent")
    if (
        not isinstance(pin, dict)
        or re.fullmatch(
            r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)",
            str(pin.get("version", "")),
        )
        is None
    ):
        raise policy.ToolError("ci.stkr.version must pin an exact release version")
    assets = pin.get("assets")
    if not isinstance(assets, dict) or not assets:
        raise policy.ToolError("ci.stkr.assets must declare platform SHA-256 checksums")
    for target, checksum in assets.items():
        if (
            target
            not in {
                "linux-x86_64",
                "linux-aarch64",
                "windows-x86_64",
                "windows-aarch64",
                "macos-x86_64",
                "macos-aarch64",
            }
            or not isinstance(checksum, str)
            or re.fullmatch(r"[0-9a-f]{64}", checksum) is None
        ):
            raise policy.ToolError("ci.stkr.assets requires supported platform names and lowercase SHA-256 checksums")
    return pin


def bootstrap_stkr(directory: Path, document: dict) -> None:
    pin = release_pin(document)
    if pin is None:
        return
    system = {"Linux": "linux", "Windows": "windows", "Darwin": "macos"}.get(platform.system())
    architecture = {
        "x86_64": "x86_64",
        "amd64": "x86_64",
        "aarch64": "aarch64",
        "arm64": "aarch64",
    }.get(platform.machine().lower())
    target = f"{system}-{architecture}"
    checksum = pin["assets"].get(target)
    if checksum is None:
        raise policy.ToolError(f"no verified stkr release asset is pinned for {target}")
    base = pin.get("api_url", "https://gitlab.com/api/v4").rstrip("/")
    parsed = urllib.parse.urlparse(base)
    if parsed.scheme != "https" or not parsed.netloc or parsed.username or parsed.password or parsed.query or parsed.fragment:
        raise policy.ToolError("ci.stkr.api_url must be an HTTPS GitLab API URL without credentials")
    project = pin.get("project_id")
    if not isinstance(project, str) or not project:
        raise policy.ToolError("ci.stkr.project_id must identify the release project")
    filename = f"stkr-{pin['version']}-{target}" + (".exe" if system == "windows" else "")
    filename = pin.get("filenames", {}).get(target, filename)
    if Path(filename).name != filename or "\\" in filename or ":" in filename or filename in {".", ".."}:
        raise policy.ToolError("ci.stkr.filenames must name exact plain release assets")
    url = f"{base}/projects/{urllib.parse.quote(project, safe='')}/packages/generic/stkr/v{pin['version']}/{filename}"
    destination = policy.inside(directory, f"target/ci-tools/{target}")
    destination.mkdir(parents=True, exist_ok=True)
    executable = destination / ("stkr.exe" if system == "windows" else "stkr")
    if executable.is_symlink():
        raise policy.ToolError("CI executable must not be a link")
    if executable.is_file() and hashlib.sha256(executable.read_bytes()).hexdigest() == checksum:
        verify_stkr(directory, executable, pin["version"])
        return
    headers = {}
    token = os.environ.get("CI_JOB_TOKEN")
    if token:
        headers["JOB-TOKEN"] = token
    request = urllib.request.Request(url, headers=headers)
    with tempfile.TemporaryDirectory(prefix=".stkr-download-", dir=destination) as temporary:
        staged = Path(temporary) / executable.name
        digest = hashlib.sha256()
        total = 0
        deadline = time.monotonic() + 30
        opener = urllib.request.build_opener(ReleaseRedirect())
        with opener.open(request, timeout=30) as response, staged.open("wb") as output:
            while chunk := response.read(1024 * 1024):
                if time.monotonic() > deadline:
                    raise policy.ToolError("stkr release download exceeded its 30 second deadline")
                total += len(chunk)
                if total > 256 * 1024 * 1024:
                    raise policy.ToolError("stkr release asset exceeds the 256 MiB download bound")
                digest.update(chunk)
                output.write(chunk)
        if digest.hexdigest() != checksum:
            raise policy.ToolError("stkr release checksum mismatch; existing executable preserved")
        staged.chmod(0o755)
        verify_stkr(directory, staged, pin["version"])
        staged.replace(executable)


def verify_stkr(directory: Path, executable: Path, version: str) -> None:
    try:
        result = subprocess.run(
            [str(executable), "--version"],
            cwd=directory,
            text=True,
            capture_output=True,
            timeout=5,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise policy.ToolError(f"could not verify stkr release executable: {error}") from error
    if result.returncode:
        raise policy.ToolError("stkr release executable version inspection failed")
    output = result.stdout
    if output.strip() != f"stkr {version}":
        raise policy.ToolError("stkr release executable version does not match its exact pin")


def run(directory: Path, verb: str) -> None:
    bootstrap_stkr(directory, policy.config(directory))


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(policy.main("release_bootstrap", ["bootstrap"], run))
