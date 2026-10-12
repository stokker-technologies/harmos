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
import json
import hashlib
import os
import platform
import re
import shutil
import subprocess
import time
from pathlib import Path

# Third Party Imports

# Local Imports
from . import _policy, evidence, release_registry, version

# Type Checking Imports

# === Public Exports === #

# === Constants === #

STABLE = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
RESERVED = {"SHA256SUMS", "release-manifest.json", "platform.json", "published.json"}

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


def tag_version(tag: str) -> str:
    if not isinstance(tag, str) or re.fullmatch("v" + STABLE, tag) is None:
        raise _policy.ToolError("release requires a stable vMAJOR.MINOR.PATCH tag")
    return tag[1:]


def target(settings: dict) -> str:
    selected = os.environ.get("RELEASE_TARGET")
    if selected not in settings.get("targets", []):
        raise _policy.ToolError("RELEASE_TARGET must be one of release.targets")
    system = {"Linux": "linux", "Windows": "windows", "Darwin": "macos"}.get(
        platform.system()
    )
    architecture = {"amd64": "x86_64", "arm64": "aarch64"}.get(
        platform.machine().lower(), platform.machine().lower()
    )
    if selected != f"{system}-{architecture}":
        raise _policy.ToolError("release target differs from the native build host")
    return selected


def tools(directory: Path, settings: dict) -> dict:
    commands = {
        "mise": ["mise", "--version"],
        "python": ["python", "--version"],
        "uv": ["uv", "--version"],
    }
    if (directory / "Cargo.toml").is_file():
        commands.update({"cargo": ["cargo", "--version"], "rustc": ["rustc", "-Vv"]})
    if (directory / "package.json").is_file():
        commands.update({"node": ["node", "--version"], "npm": ["npm", "--version"]})
    commands.update(settings.get("tools", {}))
    result = {}
    for name, command in commands.items():
        result[name] = _policy.execute(
            directory, _policy.command(command, "release.tools"), capture=True
        ).strip()
        if not result[name]:
            raise _policy.ToolError(f"release tool identity unavailable: {name}")
    return result


def assets(directory: Path, settings: dict) -> list[dict]:
    selected = target(settings)
    value = tag_version(settings["tag"])
    result = []
    for definition in settings.get("assets", []):
        if definition.get("target") != selected:
            continue
        source = _policy.inside(directory, definition["path"])
        name = definition.get("name", source.name).format(
            product=settings["product"], version=value, target=selected
        )
        if (
            Path(name).name != name
            or name in RESERVED | {".", ".."}
            or "\\" in name
            or "\n" in name
            or any(item["name"] == name for item in result)
        ):
            raise _policy.ToolError(f"invalid or duplicate release asset: {name}")
        native_package = name.endswith(".whl")
        if not native_package and not name.startswith(
            f"{settings['product']}-{value}-{selected}"
        ):
            raise _policy.ToolError("release download must name product-version-target")
        if not source.is_file() or source.is_symlink():
            raise _policy.ToolError(f"missing regular release asset: {source}")
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
        raise _policy.ToolError(f"no release assets declared for {selected}")
    return result


def prepare(directory: Path, settings: dict) -> None:
    branch = _policy.execute(
        directory, ["git", "symbolic-ref", "--short", "HEAD"], capture=True
    ).strip()
    if not branch.startswith("release/"):
        raise _policy.ToolError(
            "prepare requires a release/<version> stabilization branch"
        )
    if _policy.execute(
        directory, ["git", "status", "--porcelain"], capture=True
    ).strip():
        raise _policy.ToolError("prepare requires clean committed candidate source")
    tags = _policy.execute(
        directory,
        ["git", "tag", "--merged", "HEAD", "--list", "v*", "--sort=-v:refname"],
        capture=True,
    ).splitlines()
    if any(re.fullmatch("v" + STABLE, tag) is None for tag in tags):
        raise _policy.ToolError("prerelease tags require an explicit future policy")
    current = _policy.policy(directory, "version").get("value")
    if not tags:
        if not isinstance(current, str) or re.fullmatch(STABLE, current) is None:
            raise _policy.ToolError(
                "initial release requires the current stable version"
            )
        planned = current
    else:
        planned = (
            _policy.execute(
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
    if re.fullmatch(STABLE, planned) is None or branch != f"release/{planned}":
        raise _policy.ToolError(f"candidate branch must be release/{planned}")
    if tags:
        major, minor, patch = map(int, tags[0][1:].split("."))
        if planned != f"{major}.{minor}.{patch + 1}":
            raise _policy.ToolError(
                "git-cliff did not produce the required patch increment"
            )
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
            raise _policy.ToolError(f"prepare requires [{section}] policy")
        body = re.sub(rf"(?m)^{key}\s*=.*\n?", "", match.group(2))
        text = (
            text[: match.start()]
            + match.group(1)
            + f'{key} = "{value}"\n'
            + body
            + text[match.end() :]
        )
    policy_path.write_text(text)
    changelog = _policy.inside(directory, settings.get("notes", "CHANGELOG.md"))
    _policy.execute(
        directory, ["git-cliff", "--tag", "v" + planned, "--output", str(changelog)]
    )
    commands = settings.get("lock_commands")
    if commands is None:
        commands = []
        if (directory / "Cargo.toml").exists():
            commands.append(["cargo", "update", "--workspace", "--offline"])
        if (directory / "package.json").exists():
            commands.append(
                ["npm", "install", "--package-lock-only", "--ignore-scripts"]
            )
        for project in [directory, directory / "tools/python"]:
            if (project / "uv.lock").exists():
                commands.append(["uv", "lock", "--offline", "--project", str(project)])
    for command in commands:
        _policy.execute(directory, _policy.command(command, "release.lock_commands"))
    print(f"Prepared v{planned}; review and commit before building the candidate")


def stage(directory: Path, settings: dict, folder: Path) -> None:
    selected = assets(directory, settings)
    if _policy.policy(directory, "version").get("value") != tag_version(
        settings["tag"]
    ):
        raise _policy.ToolError("prepared package version differs from release tag")
    version.run(directory, "check")
    identity = inputs(directory)
    sha = evidence.revision(directory)
    if not identity["source_tree"] or not sha:
        raise _policy.ToolError("release staging requires clean committed source")
    provider = settings.get("provider")
    run_id = os.environ.get(
        "GITHUB_RUN_ID" if provider == "github" else "CI_PIPELINE_ID"
    )
    job_id = release_registry.job_identity(settings)
    if provider not in ["github", "gitlab"] or not run_id or not job_id:
        raise _policy.ToolError(
            "stage requires exact native provider run and job identity"
        )
    native_sha = os.environ.get(
        "GITHUB_SHA" if provider == "github" else "CI_COMMIT_SHA"
    )
    if native_sha != sha:
        raise _policy.ToolError("native build SHA differs from checkout")
    folder.mkdir(parents=True, exist_ok=True)
    receipt = folder / "platform.json"
    if receipt.exists():
        raise _policy.ToolError(
            "stage exists; explicitly clean before a recovery build"
        )
    for asset in selected:
        destination = _policy.inside(folder, asset["name"])
        if destination.exists():
            raise _policy.ToolError("stage cannot overwrite existing files")
        shutil.copy(_policy.inside(directory, asset["path"]), destination)
    _policy.write_json(
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
    report_path = _policy.inside(
        directory, settings.get("evidence", ".stkr/tests/latest.json")
    )
    report = json.loads(report_path.read_text())
    required = settings.get("tests")
    if not isinstance(required, list) or not required:
        raise _policy.ToolError(
            "release.tests must declare every required native suite"
        )
    for name in required:
        result = report.get("tests", {}).get(name, {})
        if (
            result.get("revision") != receipt["sha"]
            or result.get("outcome") != "passed"
        ):
            raise _policy.ToolError(
                f"required test evidence failed or mismatched: {name}"
            )
    receipt["validation"] = {
        "job": release_registry.job_identity(settings),
        "run_id": os.environ.get(
            "GITHUB_RUN_ID" if settings["provider"] == "github" else "CI_PIPELINE_ID"
        ),
        "sha256": hashlib.sha256(report_path.read_bytes()).hexdigest(),
        "outcome": "passed",
        "tests": required,
    }
    if (
        not receipt["validation"]["job"]
        or receipt["validation"]["run_id"] != receipt["run_id"]
    ):
        raise _policy.ToolError("test job must identify the same native run as build")
    shutil.copyfile(report_path, folder / "validation.json")
    _policy.write_json(folder / "platform.json", receipt)


def verify_platform(
    directory: Path,
    settings: dict,
    folder: Path,
    receipt: dict,
    *,
    validated: bool = True,
) -> None:
    if (
        receipt.get("sha") != evidence.revision(directory)
        or receipt.get("tag") != settings["tag"]
    ):
        raise _policy.ToolError(
            "release SHA or tag differs; new commits require a new build"
        )
    current = inputs(directory)
    # Platform environment remains in the original receipt; common locked/source inputs must match.
    for name in ["source_tree", "graph", "locks"]:
        if (
            not receipt.get("inputs", {}).get(name)
            or receipt["inputs"][name] != current[name]
        ):
            if (
                name == "graph"
                and receipt.get("inputs", {}).get(name) == current.get(name) == []
            ):
                continue
            raise _policy.ToolError(f"release resolved inputs differ: {name}")
    if not validated and (
        receipt["inputs"] != current or receipt["tools"] != tools(directory, settings)
    ):
        raise _policy.ToolError("same-SHA build environment or tools differ")
    if receipt["created_at"] > time.time() + 300:
        raise _policy.ToolError("release evidence timestamp is in the future")
    if time.time() - receipt["created_at"] > settings.get("max_age_seconds", 7 * 86400):
        raise _policy.ToolError(
            "release evidence expired; explicit recovery build required"
        )
    for asset in receipt["assets"]:
        file = _policy.inside(folder, asset["name"])
        if (
            file.is_symlink()
            or not file.is_file()
            or hashlib.sha256(file.read_bytes()).hexdigest() != asset["sha256"]
        ):
            raise _policy.ToolError(f"release asset changed: {asset['name']}")
        if asset["target"] != receipt["target"]:
            raise _policy.ToolError("asset target differs from native receipt")
    if validated:
        result = receipt.get("validation") or {}
        if (
            result.get("outcome") != "passed"
            or result.get("run_id") != receipt["run_id"]
        ):
            raise _policy.ToolError(
                "release lacks successful matching native validation"
            )
        if hashlib.sha256(
            (folder / "validation.json").read_bytes()
        ).hexdigest() != result.get("sha256"):
            raise _policy.ToolError("validation report hash differs")
        release_registry.verify_jobs(settings, receipt)


def assemble(directory: Path, settings: dict, root: Path) -> dict:
    required = settings.get("targets")
    if (
        not isinstance(required, list)
        or not required
        or len(set(required)) != len(required)
    ):
        raise _policy.ToolError(
            "release.targets must declare unique required OS/architecture targets"
        )
    selected = {}
    names = set()
    files = []
    for path in sorted(root.glob("*/platform.json")):
        receipt = json.loads(path.read_text())
        native = receipt.get("target")
        if native not in required or native in selected:
            raise _policy.ToolError("unexpected or duplicate release target")
        verify_platform(directory, settings, path.parent, receipt)
        selected[native] = receipt
        for asset in receipt["assets"]:
            if asset["name"] in names:
                raise _policy.ToolError("release download names collide across targets")
            names.add(asset["name"])
            files.append(
                {**asset, "path": str(path.parent.relative_to(root) / asset["name"])}
            )
    if set(selected) != set(required):
        raise _policy.ToolError("missing required release targets")
    manifest = {
        "schema": 1,
        "sha": evidence.revision(directory),
        "tag": settings["tag"],
        "product": settings["product"],
        "targets": selected,
        "files": files,
    }
    _policy.write_json(root / "release-manifest.json", manifest)
    sums = "".join(f"{asset['sha256']}  {asset['name']}\n" for asset in files)
    sums += f"{hashlib.sha256((root / 'release-manifest.json').read_bytes()).hexdigest()}  release-manifest.json\n"
    (root / "SHA256SUMS").write_text(sums)
    return manifest


def run(directory: Path, verb: str) -> None:
    settings = _policy.policy(directory, "release")
    if verb == "prepare":
        prepare(directory, settings)
        return
    tag_version(settings["tag"])
    if verb == "route":
        release_registry.check_route(directory, settings)
        return
    if verb == "verify":
        release_registry.verify_publication(directory, settings)
        return
    root = _policy.inside(directory, settings.get("directory", "target/release-assets"))
    if verb == "assets":
        print(json.dumps(assets(directory, settings), indent=2))
    elif verb in ["stage", "validate"]:
        folder = _policy.inside(root, target(settings))
        if verb == "stage":
            stage(directory, settings, folder)
        else:
            validate(directory, settings, folder)
    elif verb == "assemble":
        assemble(directory, settings, root)
    else:
        manifest = assemble(directory, settings, root)
        release_registry.authorize(directory, settings, manifest)
        if verb == "publish":
            release_registry.publish(directory, settings, root, manifest)
        else:
            release_registry.finalize(directory, settings, root, manifest)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(
        _policy.main(
            "release",
            [
                "prepare",
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
