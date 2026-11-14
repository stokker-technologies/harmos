# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Collect License Text From Hash-Verified Locked Python Wheels"""

# === Imports === #

# Standard Library Imports
import email.parser
import hashlib
import http.client
import io
import os
import random
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from collections.abc import Callable
from contextlib import AbstractContextManager
from pathlib import Path
from typing import BinaryIO

# Third Party Imports
# Local Imports
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

DOWNLOAD_ATTEMPTS = 6
DOWNLOAD_BASE_DELAY_S = 2
DOWNLOAD_MAX_DELAY_S = 30

# === Types === #

# === Classes === #

# === Public Functions === #


def download(
    url: str | urllib.request.Request,
    *,
    operation: str = "download",
    timeout_s: float = 60,
    opener: Callable[..., AbstractContextManager[BinaryIO]] | None = None,
    before_retry: Callable[[], None] | None = None,
) -> bytes:
    """Fetch Complete Bytes with Bounded Retries for Transient Network Failures."""
    address = url.full_url if isinstance(url, urllib.request.Request) else url
    open_request = opener or urllib.request.urlopen
    for attempt in range(1, DOWNLOAD_ATTEMPTS + 1):
        try:
            with open_request(url, timeout=timeout_s) as response:
                return response.read()
        except (OSError, urllib.error.URLError, http.client.HTTPException) as error:
            retry = not isinstance(error, urllib.error.HTTPError) or 500 <= error.code < 600
            if isinstance(error, urllib.error.HTTPError):
                error.close()
            if not retry or attempt == DOWNLOAD_ATTEMPTS:
                raise policy.ToolError(f"{operation} failed: {address} after {attempt} attempts: {error}") from error
            delay_s = min(DOWNLOAD_MAX_DELAY_S, DOWNLOAD_BASE_DELAY_S * 2 ** (attempt - 1))
            time.sleep(random.uniform(delay_s / 2, delay_s))
            if before_retry is not None:
                before_retry()
    raise AssertionError("unreachable download attempt")


def python_notices(lock: dict) -> tuple[str, int]:
    """Read License Metadata and Text from Hash-Verified Wheels Named by uv.lock."""
    sections = ["Python Locked Dependency Notices (Including Build and Development Tools)"]
    count = 0
    cache_hits = 0
    downloads = 0
    cache = Path(os.environ.get("UV_CACHE_DIR") or policy.execute(Path.cwd(), ["uv", "cache", "dir"], capture=True).strip())
    hashes = {wheel["hash"].removeprefix("sha256:") for package in lock["package"] for wheel in package.get("wheels", [])}
    cached = {}
    # uv Usually Stores Unpacked Wheels; Only Original Archives Can Be Hash-Verified.
    for directory, _, files in os.walk(cache):
        for name in files:
            if name.endswith(".whl") or name in hashes:
                cached.setdefault(name, []).append(Path(directory) / name)
    for package in lock["package"]:
        if "registry" not in package.get("source", {}):
            continue
        wheels = package.get("wheels", [])
        if not wheels:
            raise policy.ToolError(f"no locked wheel available for license inventory: {package['name']}")
        wheel = min(wheels, key=lambda item: item.get("size", 0))
        digest = wheel["hash"].removeprefix("sha256:")
        filename = Path(urllib.parse.unquote(urllib.parse.urlsplit(wheel["url"]).path)).name
        candidates = cached.get(digest, []) + cached.get(filename, [])
        data = None
        for candidate in candidates:
            contents = candidate.read_bytes()
            if hashlib.sha256(contents).hexdigest() == digest:
                data = contents
                break
        if data is None:
            data = download(wheel["url"])
            downloads += 1
        else:
            cache_hits += 1
        actual = "sha256:" + hashlib.sha256(data).hexdigest()
        if actual != wheel["hash"]:
            raise policy.ToolError(f"locked wheel hash mismatch: {package['name']} ({wheel['url']})")
        sections.append(f"\n{package['name']} {package['version']}")
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            metadata_name = next(name for name in archive.namelist() if name.endswith(".dist-info/METADATA"))
            metadata = email.parser.BytesParser().parsebytes(archive.read(metadata_name))
            license_name = metadata.get("License-Expression") or metadata.get("License")
            if not license_name:
                license_name = "; ".join(value for value in metadata.get_all("Classifier", []) if value.startswith("License ::"))
            if not license_name:
                raise policy.ToolError(f"missing Python license metadata: {package['name']}")
            sections.append(license_name)
            files = [
                name
                for name in archive.namelist()
                if not name.endswith("/")
                and (".dist-info/licenses/" in name or Path(name).name.lower().startswith(("license", "licence", "copying", "notice")))
            ]
            if not files:
                raise policy.ToolError(f"missing Python license text: {package['name']}")
            for name in sorted(files):
                sections.append(archive.read(name).decode("utf-8"))
        count += 1
    print(f"Python wheels: {cache_hits} cache hits, {downloads} downloads", file=sys.stderr)
    return "\n\n".join(sections), count


# === Private Functions === #

# === Entry Point === #
