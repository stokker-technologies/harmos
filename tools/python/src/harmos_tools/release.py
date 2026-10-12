# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Stage Verified Assets And Publish GitLab Packages And Releases"""

# === Imports === #

# Standard Library Imports
import hashlib
import json
import os
import platform
import shutil
import subprocess
import urllib.parse
import urllib.request
from pathlib import Path

# Third Party Imports

# Local Imports
from . import _policy, evidence

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def inputs(directory: Path) -> dict:
    """Identify Committed Source And Locked Native Resolved Inputs Across Promotions."""

    def output(command: list[str]) -> str:
        return subprocess.run(
            command, cwd=directory, capture_output=True, text=True, check=True
        ).stdout

    tree = output(["git", "rev-parse", "HEAD^{tree}"]).strip()
    if output(["git", "status", "--porcelain", "--untracked-files=normal"]).strip():
        tree = None
    graph = []
    if (directory / "Cargo.toml").is_file():
        metadata = json.loads(
            output(
                ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"]
            )
        )
        identities = {}
        for package in metadata["packages"]:
            source = package["source"]
            if source is None:
                manifest = Path(package["manifest_path"]).resolve()
                if not manifest.is_relative_to(directory.resolve()):
                    raise _policy.ToolError(
                        f"unpublished local dependency override: {package['name']}"
                    )
                source = "path:" + manifest.relative_to(directory.resolve()).as_posix()
            identities[package["id"]] = (
                f"{package['name']}@{package['version']}|{source}"
            )
        graph = [
            {
                "package": identities[node["id"]],
                "features": sorted(node["features"]),
                "dependencies": sorted(
                    identities[dependency] for dependency in node["dependencies"]
                ),
            }
            for node in metadata["resolve"]["nodes"]
        ]
    locks = {}
    for name in (
        "Cargo.lock",
        "mise.lock",
        "uv.lock",
        "tools/python/uv.lock",
        "pnpm-lock.yaml",
        "package-lock.json",
        "tools/python/evidence.toml",
    ):
        path = directory / name
        if path.is_file():
            locks[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    environment = {
        name: os.environ.get(name)
        for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET")
    }
    return {
        "source_tree": tree,
        "graph": sorted(graph, key=lambda node: node["package"]),
        "locks": locks,
        "build_environment": environment,
    }


def assets(directory: Path, settings: dict) -> list[dict]:
    result = []
    for definition in settings.get("assets", []):
        selected = definition.get("platform")
        if selected is not None:
            system = {"Linux": "linux", "Windows": "windows", "Darwin": "macos"}.get(
                platform.system()
            )
            architecture = {
                "x86_64": "x86_64",
                "amd64": "x86_64",
                "arm64": "aarch64",
                "aarch64": "aarch64",
            }.get(platform.machine().lower())
            supported = {
                f"{os_name}-{arch}"
                for os_name in ["linux", "windows", "macos"]
                for arch in ["x86_64", "aarch64"]
            }
            if selected not in supported:
                raise _policy.ToolError(
                    "release asset platform must name a supported operating system and architecture"
                )
            if selected != f"{system}-{architecture}":
                continue
        source = _policy.inside(directory, definition["path"])
        name = definition.get("name", source.name)
        if (
            Path(name).name != name
            or name in [".", "..", "manifest.json"]
            or any(item["name"] == name for item in result)
        ):
            raise _policy.ToolError(f"invalid or duplicate release asset: {name}")
        if not source.is_file():
            raise _policy.ToolError(f"missing release asset: {source}")
        result.append(
            {
                "name": name,
                "path": definition["path"],
                "sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                "size": source.stat().st_size,
            }
        )
    if not result:
        raise _policy.ToolError("release.assets must declare artifacts")
    return result


def request(
    settings: dict, endpoint: str, method: str, data: bytes, *, binary: bool = False
) -> dict:
    base = os.environ.get(
        "CI_API_V4_URL", settings.get("api_url", "https://gitlab.com/api/v4")
    )
    project = os.environ.get("CI_PROJECT_ID", settings.get("project_id"))
    if not project:
        raise _policy.ToolError("release.project_id or CI_PROJECT_ID is required")
    token = os.environ.get("GITLAB_TOKEN")
    job_token = os.environ.get("CI_JOB_TOKEN")
    if not token and not job_token:
        raise _policy.ToolError("GITLAB_TOKEN or CI_JOB_TOKEN is required")
    headers = {"PRIVATE-TOKEN": token} if token else {"JOB-TOKEN": job_token or ""}
    headers["Content-Type"] = (
        "application/octet-stream" if binary else "application/json"
    )
    url = f"{base}/projects/{urllib.parse.quote(str(project), safe='')}/{endpoint}"
    native = urllib.request.Request(url, data=data, headers=headers, method=method)
    with urllib.request.urlopen(native, timeout=60) as response:
        value = response.read()
    return json.loads(value) if value and not binary else {"url": url}


def run(directory: Path, verb: str) -> None:
    settings = _policy.policy(directory, "release")
    stage = _policy.inside(
        directory, settings.get("directory", "target/release-assets")
    )
    manifest = stage / "manifest.json"
    if verb == "assets":
        print(json.dumps(assets(directory, settings), indent=2))
        return
    if verb == "stage":
        selected = assets(directory, settings)
        identity = inputs(directory)
        if not identity["source_tree"]:
            raise _policy.ToolError("release staging requires committed source inputs")
        stage.mkdir(parents=True, exist_ok=True)
        manifest.unlink(missing_ok=True)
        for asset in selected:
            target = _policy.inside(stage, asset["name"])
            if target.is_symlink():
                raise _policy.ToolError("release stage must not contain links")
            shutil.copy(_policy.inside(directory, asset["path"]), target)
        _policy.write_json(
            manifest,
            {
                "revision": evidence.revision(directory),
                "inputs": identity,
                "assets": selected,
                "tools": {
                    name: evidence._query(directory, [name, "--version"])
                    for name in ("mise", "cargo", "rustc", "uv")
                },
            },
        )
        return
    staged = json.loads(manifest.read_text())
    if (
        not staged.get("revision")
        or not staged.get("inputs", {}).get("source_tree")
        or staged["inputs"] != inputs(directory)
    ):
        raise _policy.ToolError(
            "release source tree or resolved inputs differ from the validated stage"
        )
    for asset in staged["assets"]:
        path = _policy.inside(stage, asset["name"])
        if hashlib.sha256(path.read_bytes()).hexdigest() != asset["sha256"]:
            raise _policy.ToolError(f"release asset changed: {asset['name']}")
    tag = settings.get("tag", os.environ.get("CI_COMMIT_TAG"))
    if not isinstance(tag, str) or not tag:
        raise _policy.ToolError("release.tag or CI_COMMIT_TAG is required")
    if verb == "publish":
        links = []
        package = urllib.parse.quote(settings.get("package", "release"), safe="")
        version = urllib.parse.quote(tag, safe="")
        for asset in staged["assets"]:
            filename = urllib.parse.quote(asset["name"], safe="")
            endpoint = f"packages/generic/{package}/{version}/{filename}"
            response = request(
                settings,
                endpoint,
                "PUT",
                (stage / asset["name"]).read_bytes(),
                binary=True,
            )
            links.append({"name": asset["name"], "url": response["url"]})
        _policy.write_json(
            stage / "published.json",
            {"revision": staged["revision"], "tag": tag, "links": links},
        )
        return
    published = json.loads((stage / "published.json").read_text())
    if published["revision"] != staged["revision"] or published["tag"] != tag:
        raise _policy.ToolError("published assets do not match this release")
    notes = settings.get("notes")
    description = _policy.inside(directory, notes).read_text() if notes else ""
    body = {
        "tag_name": tag,
        "name": settings.get("name", tag),
        "description": description,
        "assets": {"links": published["links"]},
    }
    request(settings, "releases", "POST", json.dumps(body).encode())


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(
        _policy.main("release", ["stage", "publish", "finalize", "assets"], run)
    )
