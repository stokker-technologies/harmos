# Rendered by stkr project update from templates/python/docs/registry.py; sha256:6c6bcbd7427e956357eb4172d836199f4e4b97c26bc4850839a29b65beba5e9b
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Retain Documentation In The GitLab Generic Package Registry"""

# === Imports === #

# Standard Library Imports
import json
import os
import shutil
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

# Third Party Imports
# Local Imports
from ..release import policy
from . import artifacts

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def request(settings: dict, endpoint: str, method: str = "GET", data: bytes | None = None) -> bytes | None:
    if endpoint == "duplicate-protection":
        return protection(settings)
    api = os.environ.get("CI_API_V4_URL", settings.get("api_url", "https://gitlab.com/api/v4"))
    if not isinstance(api, str):
        raise policy.ToolError("documentation registry requires a string API URL")
    project = os.environ.get("CI_PROJECT_ID", settings.get("project_id"))
    token = os.environ.get("GITLAB_TOKEN")
    job = os.environ.get("CI_JOB_TOKEN")
    if not project or not (token or job):
        raise policy.ToolError("docs registry requires project_id/CI_PROJECT_ID and GITLAB_TOKEN/CI_JOB_TOKEN")
    if not api.startswith("https://"):
        raise policy.ToolError("documentation registry requires HTTPS")
    headers = {"PRIVATE-TOKEN": token} if token else {"JOB-TOKEN": job or ""}
    headers["Content-Type"] = "application/octet-stream"
    url = f"{api}/projects/{urllib.parse.quote(str(project), safe='')}/{endpoint}"
    native = urllib.request.Request(url, data=data, method=method, headers=headers)
    try:
        with urllib.request.urlopen(native, timeout=60) as response:
            return response.read()
    except urllib.error.HTTPError as error:
        if error.code == 404 and method == "GET":
            return None
        raise policy.ToolError(f"documentation registry {method} failed: HTTP {error.code}") from error


def protection(settings: dict) -> bytes:
    token = os.environ.get("GITLAB_TOKEN")
    if not token:
        raise policy.ToolError("GITLAB_TOKEN with API read access is required to verify namespace duplicate protection")
    project = request(settings, "")
    if project is None:
        raise policy.ToolError("registry project metadata is unavailable")
    namespace = json.loads(project)["namespace"]["full_path"].split("/")[0]
    api = os.environ.get("CI_API_V4_URL", settings.get("api_url", "https://gitlab.com/api/v4"))
    if not isinstance(api, str):
        raise policy.ToolError("documentation registry requires a string API URL")
    query = (
        "query($path: ID!) { namespace(fullPath: $path) { packageSettings { genericDuplicatesAllowed genericDuplicateExceptionRegex } } }"
    )
    body = json.dumps({"query": query, "variables": {"path": namespace}}).encode()
    native = urllib.request.Request(
        api.removesuffix("/v4") + "/graphql",
        data=body,
        headers={"PRIVATE-TOKEN": token, "Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(native, timeout=60) as response:
        result = json.loads(response.read())
    if result.get("errors") or not result.get("data", {}).get("namespace"):
        raise policy.ToolError("namespace duplicate protection could not be verified")
    settings = result["data"]["namespace"]["packageSettings"]
    if not settings:
        raise policy.ToolError("namespace package settings are unavailable")
    return json.dumps(
        {
            "generic_duplicates_allowed": settings["genericDuplicatesAllowed"],
            "generic_duplicate_exception_regex": settings.get("genericDuplicateExceptionRegex"),
        }
    ).encode()


def endpoint(settings: dict, version: str) -> str:
    package = urllib.parse.quote(settings.get("package", "documentation"), safe="")
    return f"packages/generic/{package}/{urllib.parse.quote(version, safe='')}/site.zip"


def retained(settings: dict, destination: Path) -> list[tuple[Path, dict]]:
    versions = set()
    page = 1
    name = settings.get("package", "documentation")
    while True:
        query = urllib.parse.urlencode(
            {
                "package_type": "generic",
                "package_name": name,
                "per_page": 100,
                "page": page,
            }
        )
        response = request(settings, "packages?" + query)
        if response is None:
            raise policy.ToolError("documentation package listing is unavailable")
        records = json.loads(response)
        for item in records:
            if item["name"] == name and item.get("status", "default") == "default":
                versions.add(item["version"])
        if len(records) < 100:
            break
        page += 1
    results = []
    development = []
    for version in sorted(versions):
        if not version.startswith(("v", "development-")):
            raise policy.ToolError(f"unrecognized documentation package version: {version}")
        data = request(settings, endpoint(settings, version))
        if data is None:
            raise policy.ToolError(f"published documentation archive missing: {version}")
        folder = policy.inside(destination, version)
        manifest = artifacts.unpack(data, folder)
        if version.startswith("development-"):
            if manifest["selector"] != "development" or version != "development-" + manifest["revision"]:
                raise policy.ToolError("development package identity differs")
            development.append((folder, manifest))
        elif manifest["selector"] != version:
            raise policy.ToolError("release package identity differs")
        else:
            results.append((folder, manifest))
    if development:
        # Main Ancestry Is Verified During Build; Registry IDs Determine Development Upload Order Only.
        chosen = max(development, key=lambda item: item[1]["built_at"])
        results.append(chosen)
    return results


def publish(settings: dict, folder: Path, manifest: dict) -> str:
    data = artifacts.archive(folder, manifest)
    version = manifest["selector"] if manifest["selector"] != "development" else "development-" + manifest["revision"]
    path = endpoint(settings, version)
    previous = request(settings, path)
    if previous is not None:
        previous_folder = folder.parent / "collision"
        if previous_folder.exists():
            shutil.rmtree(previous_folder)
        previous_manifest = artifacts.unpack(previous, previous_folder)
        if previous_manifest != manifest:
            raise policy.ToolError(f"immutable documentation version collision: {version}")
        return artifacts.digest(previous)
    # GitLab Can Accept Duplicate Files: Require Its Native Duplicate Protection.
    response = request(settings, "duplicate-protection")
    if response is None or (
        json.loads(response).get("generic_duplicates_allowed", True) or json.loads(response).get("generic_duplicate_exception_regex")
    ):
        raise policy.ToolError("disable generic package duplicates in GitLab before documentation publication")
    request(settings, path, "PUT", data)
    uploaded = request(settings, path)
    if uploaded is None or artifacts.digest(uploaded) != artifacts.digest(data):
        raise policy.ToolError("registry documentation identity differs after upload")
    return artifacts.digest(data)


# === Private Functions === #

# === Entry Point === #
