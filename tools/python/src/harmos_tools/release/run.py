# Rendered by stkr project update from templates/python/release/run.py; sha256:811ae46e3b8574f92daf3f1838c5f98a6dc2b5339ffed357dfb43fb093aa92bf
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Prepare Candidates And Bind Native Validation To Immutable Releases"""

# === Imports === #

# Standard Library Imports
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import time
import urllib.parse
from functools import cache
from pathlib import Path

# Third Party Imports
# Local Imports
# isort: split
from ..test import archive, run as test
from . import policy, registry, screenshots, version

# Type Checking Imports

# === Public Exports === #

# === Constants === #

STABLE = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
RESERVED = {"SHA256SUMS", "release.json", "platform.json", "published.json"}
LOCKS = (
    "*Cargo.lock",
    "*mise.lock",
    "*uv.lock",
    "*pnpm-lock.yaml",
    "*package-lock.json",
)

# === Types === #

# === Classes === #

# === Public Functions === #


def inputs(directory: Path) -> dict:
    """Identify Committed Source And Locked Native Resolved Inputs Across Promotions."""

    def output(command: list[str]) -> str:
        return subprocess.run(
            command,
            cwd=directory,
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        ).stdout

    tree = output(["git", "rev-parse", "HEAD^{tree}"]).strip()
    if output(["git", "status", "--porcelain", "--untracked-files=normal"]).strip():
        tree = None
    graph = []
    overrides = []
    if (directory / "Cargo.toml").is_file():
        metadata = json.loads(output(["cargo", "metadata", "--locked", "--offline", "--format-version", "1"]))
        identities = {}
        for package in metadata["packages"]:
            source = package["source"]
            if source is None:
                manifest = Path(package["manifest_path"]).resolve()
                if manifest.is_relative_to(directory.resolve()):
                    relative = manifest.relative_to(directory.resolve()).as_posix()
                    source = "path:" + relative
                else:
                    overrides.append(
                        {
                            "package": package["name"],
                            "manifest": manifest.as_posix(),
                            "source": _override_source(manifest.parent),
                        }
                    )
                    source = "local:" + manifest.as_posix()
            identities[package["id"]] = f"{package['name']}@{package['version']}|{source}"
        graph = [
            {
                "package": identities[node["id"]],
                "features": sorted(node["features"]),
                "dependencies": sorted(identities[dependency] for dependency in node["dependencies"]),
            }
            for node in metadata["resolve"]["nodes"]
        ]
    # Tooling Lives With Its Package, So Every Tracked Lock File Counts.
    locks = {}
    for name in _tracked(directory, *LOCKS):
        path = directory / name
        if path.is_file():
            locks[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    environment = {name: os.environ.get(name) for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET")}
    identity = {
        "source_tree": tree,
        "graph": sorted(graph, key=lambda node: node["package"]),
        "locks": locks,
        "build_environment": environment,
    }

    # Local Overrides Mark The Identity Unpublishable; CI Refuses Them Outright.
    if overrides:
        overrides.sort(key=lambda override: (override["package"], override["manifest"]))
        if continuous_integration():
            names = ", ".join(override["package"] for override in overrides)
            raise policy.ToolError(f"unpublished local dependency override: {names}")
        _warn_overrides(tuple((item["package"], item["manifest"]) for item in overrides))
        identity["local_overrides"] = overrides
    return identity


def continuous_integration() -> bool:
    """Whether a CI runner executes this process."""
    return any(os.environ.get(name, "").lower() in {"1", "true"} for name in ("CI", "GITHUB_ACTIONS", "GITLAB_CI"))


def tag_version(tag: str) -> str:
    if not isinstance(tag, str) or re.fullmatch("v" + STABLE, tag) is None:
        raise policy.ToolError("release requires a stable vMAJOR.MINOR.PATCH tag")
    return tag[1:]


def target(settings: dict) -> str:
    selected = os.environ.get("RELEASE_TARGET")
    if not isinstance(selected, str) or selected not in settings.get("targets", []):
        raise policy.ToolError("RELEASE_TARGET must be one of release.targets")
    system = {"Linux": "linux", "Windows": "windows", "Darwin": "macos"}.get(platform.system())
    architecture = {"amd64": "x86_64", "arm64": "aarch64"}.get(platform.machine().lower(), platform.machine().lower())
    if selected != f"{system}-{architecture}":
        raise policy.ToolError("release target differs from the native build host")
    return selected


def tools(directory: Path, settings: dict) -> dict:
    commands = {
        "mise": ["mise", "--version"],
        "python": ["python", "--version"],
        "uv": ["uv", "--version"],
    }
    if (directory / "Cargo.toml").is_file():
        commands.update({"cargo": ["cargo", "--version"], "rustc": ["rustc", "-Vv"]})
    if _tracked(directory, "*package.json"):
        commands.update({"node": ["node", "--version"], "npm": ["npm", "--version"]})
    commands.update(settings.get("tools", {}))
    result = {}
    for name, command in commands.items():
        result[name] = policy.execute(directory, policy.command(command, "release.tools"), capture=True).strip()
        if not result[name]:
            raise policy.ToolError(f"release tool identity unavailable: {name}")
    return result


def assets(directory: Path, settings: dict) -> list[dict]:
    selected = target(settings)
    value = tag_version(settings["tag"])
    result = []
    for definition in settings.get("assets", []):
        if definition.get("target") != selected:
            continue
        source = policy.inside(directory, definition["path"])
        name = definition.get("name", source.name).format(product=settings["product"], version=value, target=selected)
        if (
            Path(name).name != name
            or name in RESERVED | {".", ".."}
            or "\\" in name
            or "\n" in name
            or any(item["name"] == name for item in result)
        ):
            raise policy.ToolError(f"invalid or duplicate release asset: {name}")
        native_package = name.endswith(".whl")
        if not native_package and not name.startswith(f"{settings['product']}-{value}-{selected}"):
            raise policy.ToolError("release download must name product-version-target")
        if not source.is_file() or source.is_symlink():
            raise policy.ToolError(f"missing regular release asset: {source}")
        result.append(
            {
                "name": name,
                "path": definition["path"],
                "target": selected,
                "sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                "size": source.stat().st_size,
            }
        )
    if not result:
        raise policy.ToolError(f"no release assets declared for {selected}")
    return result


def prepare(directory: Path, settings: dict) -> None:
    branch = policy.execute(directory, ["git", "symbolic-ref", "--short", "HEAD"], capture=True).strip()
    if not branch.startswith("release/v"):
        raise policy.ToolError("prepare requires a release/v<version> stabilization branch")
    if policy.execute(directory, ["git", "status", "--porcelain"], capture=True).strip():
        raise policy.ToolError("prepare requires clean committed candidate source")
    tags = policy.execute(
        directory,
        ["git", "tag", "--merged", "HEAD", "--list", "v*", "--sort=-v:refname"],
        capture=True,
    ).splitlines()
    if any(re.fullmatch("v" + STABLE, tag) is None for tag in tags):
        raise policy.ToolError("prerelease tags require an explicit future policy")
    current = policy.policy(directory, "version").get("value")
    if not tags:
        if not isinstance(current, str) or re.fullmatch(STABLE, current) is None:
            raise policy.ToolError("initial release requires the current stable version")
        planned = current
    else:
        planned = (
            policy.execute(
                directory,
                [
                    "git-cliff",
                    "--bump",
                    "patch",
                    "--bumped-version",
                    "--use-branch-tags",
                ],
                capture=True,
            )
            .strip()
            .removeprefix("v")
        )
    if re.fullmatch(STABLE, planned) is None or branch != f"release/v{planned}":
        raise policy.ToolError(f"candidate branch must be release/v{planned}")
    if tags:
        major, minor, patch = map(int, tags[0][1:].split("."))
        if planned != f"{major}.{minor}.{patch + 1}":
            raise policy.ToolError("git-cliff did not produce the required patch increment")
    version.run(directory, "sync", value=planned)
    policy_path = directory / "stkr.toml"
    text = policy_path.read_text()
    for section, key, value in [
        ("version", "value", planned),
        ("release", "tag", "v" + planned),
    ]:
        pattern = rf"(?ms)(^\[{section}\]\s*\n)(.*?)(?=^\[|\Z)"
        match = re.search(pattern, text)
        if match is None:
            raise policy.ToolError(f"prepare requires [{section}] policy")
        body = re.sub(rf"(?m)^{key}\s*=.*\n?", "", match.group(2))
        text = text[: match.start()] + match.group(1) + f'{key} = "{value}"\n' + body + text[match.end() :]
    policy_path.write_text(text)
    changelog = policy.inside(directory, settings.get("notes", "CHANGELOG.md"))
    policy.execute(directory, ["git-cliff", "--tag", "v" + planned, "--output", str(changelog)])
    commands = settings.get("lock_commands")
    if commands is None:
        commands = []
        if (directory / "Cargo.toml").exists():
            commands.append(["cargo", "update", "--workspace", "--offline"])
        # Each Lock Is Refreshed Beside The Package That Owns It.
        for lock in _tracked(directory, "*package-lock.json"):
            package = str(directory / Path(lock).parent)
            commands.append(
                [
                    "npm",
                    "install",
                    "--package-lock-only",
                    "--ignore-scripts",
                    "--prefix",
                    package,
                ]
            )
        for lock in _tracked(directory, "*pnpm-lock.yaml"):
            package = str(directory / Path(lock).parent)
            commands.append(
                [
                    "pnpm",
                    "install",
                    "--lockfile-only",
                    "--ignore-scripts",
                    "--dir",
                    package,
                ]
            )
        for lock in _tracked(directory, "*uv.lock"):
            project = str(directory / Path(lock).parent)
            commands.append(["uv", "lock", "--offline", "--project", project])
    for command in commands:
        policy.execute(directory, policy.command(command, "release.lock_commands"))
    if policy.policy(directory, "screenshots").get("scenes"):
        screenshots.capture(directory)
    print(f"Prepared v{planned}; review and commit before building the candidate")


def candidate(directory: Path, settings: dict) -> None:
    """Prepare Or Resume One Stable Candidate From Published Dev And Open Its Review."""
    version = os.environ.get("RELEASE_VERSION", "")
    tag_version("v" + version)
    github = settings["provider"] == "github"
    if (
        github
        and os.environ.get("GITHUB_EVENT_NAME") != "workflow_dispatch"
        or not github
        and os.environ.get("CI_PIPELINE_SOURCE") != "web"
    ):
        raise policy.ToolError("candidate authoring requires the native manual release workflow")
    if policy.execute(directory, ["git", "status", "--porcelain"], capture=True).strip():
        raise policy.ToolError("candidate authoring requires a clean checkout")
    remote = settings.get("remote", "origin")
    branch = "release/v" + version
    policy.execute(directory, ["git", "fetch", remote, "dev"])
    development = policy.execute(directory, ["git", "rev-parse", "FETCH_HEAD"], capture=True).strip()
    existing = policy.execute(directory, ["git", "ls-remote", "--heads", remote, "refs/heads/" + branch], capture=True).strip()
    if existing:
        policy.execute(directory, ["git", "fetch", remote, branch])
        revision = policy.execute(directory, ["git", "rev-parse", "FETCH_HEAD"], capture=True).strip()
        local = policy.execute(directory, ["git", "branch", "--list", branch], capture=True).strip()
        if local:
            policy.execute(directory, ["git", "switch", branch])
            if archive.revision(directory) != revision:
                raise policy.ToolError("local candidate differs from its published branch; refuse to reset it")
        else:
            policy.execute(directory, ["git", "switch", "--create", branch, revision])
        if policy.policy(directory, "version").get("value") != version or policy.policy(directory, "release").get("tag") != "v" + version:
            raise policy.ToolError("existing release branch has another prepared version")
    else:
        policy.execute(directory, ["git", "switch", "--create", branch, development])
        prepare(directory, settings)
        policy.execute(directory, ["git", "add", "--all"])
        if policy.execute(directory, ["git", "status", "--porcelain"], capture=True).strip():
            author = os.environ.get("GITHUB_ACTOR", os.environ.get("GITLAB_USER_NAME", "Release Maintainer"))
            email = os.environ.get("GITLAB_USER_EMAIL", author + "@users.noreply.github.com")
            policy.execute(
                directory,
                ["git", "-c", "user.name=" + author, "-c", "user.email=" + email, "commit", "-m", "chore: prepare release v" + version],
            )
    policy.execute(directory, ["mise", "run", "ci"])
    if policy.execute(directory, ["git", "status", "--porcelain"], capture=True).strip():
        raise policy.ToolError("candidate CI changed source; commit a coherent repair before retrying")
    destination = remote
    if not github:
        identity = registry.document(settings, "")
        if not identity or str(identity.get("id")) != os.environ.get("CI_PROJECT_ID") or not os.environ.get("GITLAB_TOKEN"):
            raise policy.ToolError("candidate push requires the native protected project token")
        destination = f"https://oauth2:{os.environ['GITLAB_TOKEN']}@{os.environ['CI_SERVER_HOST']}/{os.environ['CI_PROJECT_PATH']}.git"
    policy.execute(directory, ["git", "push", destination, "HEAD:refs/heads/" + branch])
    query = (
        "state=open&head="
        + urllib.parse.quote(os.environ.get("GITHUB_REPOSITORY", settings.get("repository", "")).split("/")[0] + ":" + branch, safe="")
        + "&base=main"
        if github
        else "state=opened&source_branch=" + urllib.parse.quote(branch, safe="") + "&target_branch=main"
    )
    endpoint = "pulls" if github else "merge_requests"
    proposals = registry.pages(settings, endpoint + "?" + query)
    if len(proposals) > 1:
        raise policy.ToolError("multiple open reviews identify the release branch")
    if not proposals:
        body = (
            {"title": "Release v" + version, "body": "Prepare Stable Release v" + version, "head": branch, "base": "main"}
            if github
            else {
                "title": "Release v" + version,
                "description": "Prepare Stable Release v" + version,
                "source_branch": branch,
                "target_branch": "main",
                "squash": False,
                "remove_source_branch": False,
            }
        )
        proposals = [registry.document(settings, endpoint, "POST", body)]
    print(proposals[0].get("html_url", proposals[0].get("web_url", "")))


def tag(directory: Path, settings: dict) -> None:
    """Tag One Verified Main Merge Idempotently, Without Publishing A Release."""
    sha = archive.revision(directory)
    github = settings["provider"] == "github"
    if not sha or policy.execute(directory, ["git", "status", "--porcelain"], capture=True).strip():
        raise policy.ToolError("tagging requires clean committed main source")
    if github:
        valid = (
            os.environ.get("GITHUB_EVENT_NAME") == "push"
            and os.environ.get("GITHUB_REF") == "refs/heads/main"
            and os.environ.get("GITHUB_SHA") == sha
        )
    else:
        valid = (
            os.environ.get("CI_PIPELINE_SOURCE") == "push"
            and os.environ.get("CI_COMMIT_BRANCH") == "main"
            and os.environ.get("CI_COMMIT_SHA") == sha
            and os.environ.get("CI_COMMIT_REF_PROTECTED") == "true"
        )
    if not valid or settings["tag"] != "v" + str(policy.policy(directory, "version").get("value")):
        raise policy.ToolError("tagging requires the prepared version on a protected native main push")
    registry.merged_review(directory, settings, sha)
    name = settings["tag"]
    endpoint = "git/ref/tags/" + name if github else "repository/tags/" + name
    existing = registry.document(settings, endpoint)
    if existing:
        if github:
            object = existing.get("object", {})
            if object.get("type") == "tag":
                object = registry.document(settings, "git/tags/" + object["sha"]).get("object", {})
            revision = object.get("sha")
        else:
            revision = existing.get("commit", {}).get("id")
        if revision != sha:
            raise policy.ToolError("existing release tag identifies another revision; never move it")
        return
    endpoint = "git/refs" if github else "repository/tags"
    body = {"ref": "refs/tags/" + name, "sha": sha} if github else {"tag_name": name, "ref": sha}
    registry.document(settings, endpoint, "POST", body)


def stage(directory: Path, settings: dict, folder: Path) -> None:
    registry.require_tag(directory, settings)
    selected = assets(directory, settings)
    if policy.policy(directory, "version").get("value") != tag_version(settings["tag"]):
        raise policy.ToolError("prepared package version differs from release tag")
    version.run(directory, "check")
    identity = inputs(directory)
    if identity.get("local_overrides"):
        raise policy.ToolError("release staging rejects local dependency overrides")
    sha = archive.revision(directory)
    if not identity["source_tree"] or not sha:
        raise policy.ToolError("release staging requires clean committed source")
    provider = settings.get("provider")
    run_id = os.environ.get("GITHUB_RUN_ID" if provider == "github" else "CI_PIPELINE_ID")
    job_id = registry.job_identity(settings)
    if provider not in ["github", "gitlab"] or not run_id or not job_id:
        raise policy.ToolError("stage requires exact native provider run and job identity")
    native_sha = os.environ.get("GITHUB_SHA" if provider == "github" else "CI_COMMIT_SHA")
    if native_sha != sha:
        raise policy.ToolError("native build SHA differs from checkout")
    folder.mkdir(parents=True, exist_ok=True)
    receipt = folder / "platform.json"
    if receipt.exists():
        raise policy.ToolError("stage exists; explicitly clean before a recovery build")
    for asset in selected:
        destination = policy.inside(folder, asset["name"])
        if destination.exists():
            raise policy.ToolError("stage cannot overwrite existing files")
        shutil.copy(policy.inside(directory, asset["path"]), destination)
    policy.write_json(
        receipt,
        {
            "schema": 1,
            "sha": sha,
            "tag": settings["tag"],
            "target": target(settings),
            "provider": provider,
            "run_id": run_id,
            "build_job": job_id,
            "attempt": os.environ.get("GITHUB_RUN_ATTEMPT", "1"),
            "inputs": identity,
            "tools": tools(directory, settings),
            "assets": selected,
            "validation": None,
            "created_at": int(time.time()),
        },
    )


def validate(directory: Path, settings: dict, folder: Path) -> None:
    receipt = json.loads((folder / "platform.json").read_text())
    verify_platform(directory, settings, folder, receipt, validated=False)
    report_path = policy.inside(directory, test.MANIFEST.as_posix())
    required = settings.get("tests")
    if not isinstance(required, list) or not required:
        raise policy.ToolError("release.tests must declare every required native suite")
    missing = test.missing(directory, required, receipt["sha"])
    if missing:
        raise policy.ToolError("required test evidence failed or mismatched: " + ", ".join(missing))
    receipt["validation"] = {
        "job": registry.job_identity(settings),
        "run_id": os.environ.get("GITHUB_RUN_ID" if settings["provider"] == "github" else "CI_PIPELINE_ID"),
        "sha256": hashlib.sha256(report_path.read_bytes()).hexdigest(),
        "outcome": "passed",
        "tests": required,
    }
    if not receipt["validation"]["job"] or receipt["validation"]["run_id"] != receipt["run_id"]:
        raise policy.ToolError("test job must identify the same native run as build")
    shutil.copyfile(report_path, folder / "validation.json")
    policy.write_json(folder / "platform.json", receipt)


def verify_platform(
    directory: Path,
    settings: dict,
    folder: Path,
    receipt: dict,
    *,
    validated: bool = True,
) -> None:
    if receipt.get("sha") != archive.revision(directory) or receipt.get("tag") != settings["tag"]:
        raise policy.ToolError("release SHA or tag differs; new commits require a new build")
    current = inputs(directory)
    if current.get("local_overrides") or receipt.get("inputs", {}).get("local_overrides"):
        raise policy.ToolError("release verification rejects local dependency overrides")
    # Platform environment remains in the original receipt; common locked/source inputs must match.
    for name in ["source_tree", "graph", "locks"]:
        if not receipt.get("inputs", {}).get(name) or receipt["inputs"][name] != current[name]:
            if name == "graph" and receipt.get("inputs", {}).get(name) == current.get(name) == []:
                continue
            raise policy.ToolError(f"release resolved inputs differ: {name}")
    if not validated and (receipt["inputs"] != current or receipt["tools"] != tools(directory, settings)):
        raise policy.ToolError("same-SHA build environment or tools differ")
    if receipt["created_at"] > time.time() + 300:
        raise policy.ToolError("release evidence timestamp is in the future")
    if time.time() - receipt["created_at"] > settings.get("max_age_seconds", 7 * 86400):
        raise policy.ToolError("release evidence expired; explicit recovery build required")
    for asset in receipt["assets"]:
        file = policy.inside(folder, asset["name"])
        if file.is_symlink() or not file.is_file() or hashlib.sha256(file.read_bytes()).hexdigest() != asset["sha256"]:
            raise policy.ToolError(f"release asset changed: {asset['name']}")
        if asset["target"] != receipt["target"]:
            raise policy.ToolError("asset target differs from native receipt")
    if validated:
        result = receipt.get("validation") or {}
        if result.get("outcome") != "passed" or result.get("run_id") != receipt["run_id"]:
            raise policy.ToolError("release lacks successful matching native validation")
        if hashlib.sha256((folder / "validation.json").read_bytes()).hexdigest() != result.get("sha256"):
            raise policy.ToolError("validation report hash differs")
        registry.verify_jobs(settings, receipt)


def assemble(directory: Path, settings: dict, root: Path) -> dict:
    required = settings.get("targets")
    if not isinstance(required, list) or not required or len(set(required)) != len(required):
        raise policy.ToolError("release.targets must declare unique required OS/architecture targets")
    selected = {}
    names = set()
    files = []
    for path in sorted(root.glob("*/platform.json")):
        receipt = json.loads(path.read_text())
        native = receipt.get("target")
        if native not in required or native in selected:
            raise policy.ToolError("unexpected or duplicate release target")
        verify_platform(directory, settings, path.parent, receipt)
        selected[native] = receipt
        for asset in receipt["assets"]:
            if asset["name"] in names:
                raise policy.ToolError("release download names collide across targets")
            names.add(asset["name"])
            files.append({**asset, "path": str(path.parent.relative_to(root) / asset["name"])})
    if set(selected) != set(required):
        raise policy.ToolError("missing required release targets")
    manifest = {
        "schema": 1,
        "sha": archive.revision(directory),
        "tag": settings["tag"],
        "product": settings["product"],
        "targets": selected,
        "files": files,
    }
    policy.write_json(root / "release.json", manifest)
    sums = "".join(f"{asset['sha256']}  {asset['name']}\n" for asset in files)
    sums += f"{hashlib.sha256((root / 'release.json').read_bytes()).hexdigest()}  release.json\n"
    (root / "SHA256SUMS").write_text(sums)
    return manifest


def run(directory: Path, verb: str) -> None:
    settings = policy.policy(directory, "release")
    if verb == "candidate":
        candidate(directory, settings)
        return
    if verb == "prepare":
        prepare(directory, settings)
        return
    tag_version(settings["tag"])
    if verb == "tag":
        tag(directory, settings)
        return
    if verb == "guard":
        registry.require_tag(directory, settings)
        return
    if verb == "route":
        registry.check_route(directory, settings)
        return
    if verb == "verify":
        registry.verify_publication(directory, settings)
        return
    root = policy.inside(directory, settings.get("directory", "target/release-assets"))
    if verb == "assets":
        print(json.dumps(assets(directory, settings), indent=2))
    elif verb in ["stage", "validate"]:
        folder = policy.inside(root, target(settings))
        if verb == "stage":
            stage(directory, settings, folder)
        else:
            validate(directory, settings, folder)
    elif verb == "assemble":
        assemble(directory, settings, root)
    else:
        manifest = assemble(directory, settings, root)
        registry.authorize(directory, settings, manifest)
        if verb == "publish":
            registry.publish(directory, settings, root, manifest)
        else:
            registry.finalize(directory, settings, root, manifest)


# === Private Functions === #


def _override_source(directory: Path) -> str:
    """Bind Local Development Evidence To The External Checkout Contents."""
    root = Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=directory,
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        ).stdout.strip()
    )
    files = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=root,
        capture_output=True,
        check=True,
    ).stdout.split(b"\0")
    digest = hashlib.sha256()
    for name in sorted(set(files) - {b""}):
        path = root / os.fsdecode(name)
        digest.update(name + b"\0")
        if path.is_symlink():
            digest.update(b"link\0" + os.fsencode(os.readlink(path)))
        elif path.is_file():
            digest.update(b"file\0" + hashlib.sha256(path.read_bytes()).digest())
        else:
            digest.update(b"missing\0")
    return digest.hexdigest()


@cache
def _tracked(directory: Path, *patterns: str) -> list[str]:
    """Tracked paths matching Git pathspec patterns, where `*` also crosses directories."""
    listed = policy.execute(directory, ["git", "ls-files", "-z", "--", *patterns], capture=True)
    return sorted(filter(None, listed.split("\0")))


def _warn_overrides(overrides: tuple[tuple[str, str], ...]) -> None:
    """Warn once per process that local overrides make evidence unpublishable."""
    listing = "\n".join(f"  {package}: {manifest}" for package, manifest in overrides)
    print(
        f"warning: unpublished local dependency overrides; local evidence is not publishable and CI refuses this tree:\n{listing}",
        file=sys.stderr,
    )


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(
        policy.main(
            "release",
            [
                "guard",
                "prepare",
                "candidate",
                "tag",
                "stage",
                "validate",
                "assemble",
                "publish",
                "finalize",
                "verify",
                "route",
                "assets",
            ],
            run,
        )
    )
