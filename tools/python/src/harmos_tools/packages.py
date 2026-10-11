# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Stage And Verify Exact Native Release Packages"""

# === Imports === #

# Standard Library Imports
import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import tomllib
import zipfile
from pathlib import Path, PurePosixPath
from urllib.error import HTTPError
from urllib.request import Request, urlopen

# Third Party Imports

# Local Imports
from . import evidence, release

# Type Checking Imports

# === Public Exports === #

# === Constants === #

BINARIES = (
    "harmos-component",
    "harmos-sidecar-export",
    "minischematic-cli",
    "minischematic-worker",
    "minibench-cli",
    "minibench-probe-sidecar",
    "standalone-sidecar",
)
PLUGINS = {
    "minischematic_plugin": "plugins/module.wasm",
    "autoroute": "plugins/component.wasm",
}

VERSION_COMMANDS = {
    "rust": ["rustc", "-vV"],
    "cargo": ["cargo", "--version"],
    "python": ["python", "--version"],
    "uv": ["uv", "--version"],
}

# === Types === #

# === Classes === #

# === Public Functions === #


def platform() -> str:
    if os.name == "nt":
        return "windows"
    if sys.platform == "linux":
        return "linux"
    raise RuntimeError(f"undeclared release platform {sys.platform}")


def location(repository: Path) -> Path:
    return repository / "dist" / platform()


def source(repository: Path) -> str:
    # Inspect Product And Tooling Content Without Reading Restricted Documentation
    result = subprocess.run(
        [
            "git",
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            ".",
            ":(exclude)docs/**",
        ],
        cwd=repository,
        check=True,
        capture_output=True,
        env=_git_environment(),
    )
    digest = hashlib.sha256()
    for name in sorted(set(result.stdout.decode().split("\0")) - {""}):
        path = repository / name
        digest.update(name.encode() + b"\0")
        if path.is_symlink():
            digest.update(os.readlink(path).encode())
        elif path.is_file():
            digest.update(path.read_bytes())
        else:
            digest.update(b"deleted")
    return digest.hexdigest()


def stage(repository: Path) -> None:
    inputs = release.inputs(repository)
    if os.environ.get("CI") == "true" and not inputs["source_tree"]:
        raise RuntimeError("CI release packaging requires clean committed source")
    root = location(repository)
    if root.exists():
        shutil.rmtree(root)
    (root / "cli").mkdir(parents=True)
    (root / "plugins").mkdir()
    (root / "reports").mkdir()
    shutil.copy2(repository / "LICENSE-APACHE", root / "LICENSE-APACHE")
    target = _target(repository)
    suffix = ".exe" if os.name == "nt" else ""
    artifacts = {}
    for name in BINARIES:
        relative = f"cli/{name}{suffix}"
        shutil.copy2(target / "release" / f"{name}{suffix}", root / relative)
        artifacts[name] = relative
    core = target / "wasm32-unknown-unknown/release/minischematic_plugin.wasm"
    shutil.copy2(core, root / PLUGINS["minischematic_plugin"])
    subprocess.run(
        [
            str(root / artifacts["harmos-component"]),
            str(root / PLUGINS["minischematic_plugin"]),
            str(root / PLUGINS["autoroute"]),
        ],
        cwd=repository,
        check=True,
    )
    artifacts.update(PLUGINS)
    # Original Build Provenance Survives Squash And Promotion
    revision = _output(repository, ["git", "rev-parse", "HEAD"])
    versions = {
        name: _output(repository, command) for name, command in VERSION_COMMANDS.items()
    }
    versions["mise"] = _output(repository, ["mise", "--version"])
    manifest = {
        "schema": 2,
        "platform": platform(),
        "built_unix_seconds": int(time.time()),
        "run": os.environ.get("GITHUB_RUN_ID"),
        "attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
        "job": os.environ.get("GITHUB_JOB"),
        "revision": revision,
        "source": source(repository),
        "inputs": inputs,
        "versions": versions,
        "artifacts": artifacts,
        "digests": {
            relative: _digest(root / relative) for relative in artifacts.values()
        },
    }
    (root / "reports/build.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )
    for name in ("Cargo.lock", "mise.lock", "tools/python/uv.lock"):
        shutil.copy2(repository / name, root / "reports" / Path(name).name)
    timings = target / "cargo-timings"
    if timings.is_dir():
        shutil.copytree(timings, root / "reports/cargo-timings")
    verify(repository)
    export(repository)


def verify(repository: Path) -> dict[str, Path]:
    return _verify(repository, location(repository))


def restore(repository: Path) -> None:
    paths = verify(repository)
    # Native Actions Transfer Preserves Bytes But Drops Executable Permissions
    if os.name != "nt":
        for name in BINARIES:
            paths[name].chmod(0o755)


def export(repository: Path) -> None:
    root = location(repository)
    if not root.is_dir():
        raise FileNotFoundError(f"release package {root} is absent")
    # Failure Diagnostics Remain Downloadable Without Claiming Validation Success
    reports = repository / ".stkr/tests"
    if reports.is_dir():
        destination = root / "tests"
        if destination.exists():
            shutil.rmtree(destination)
        shutil.copytree(reports, destination)
    diagnostics = repository / "target/validation"
    if diagnostics.is_dir():
        destination = root / "reports/test-diagnostics"
        if destination.exists():
            shutil.rmtree(destination)
        for path in diagnostics.rglob("*"):
            relative = path.relative_to(diagnostics)
            if (
                not path.is_file()
                or "cargo-target" in relative.parts
                or path.suffix
                not in {".xml", ".json", ".log", ".html", ".css", ".js", ".svg"}
            ):
                continue
            output = destination / relative
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, output)
    archive = repository / "dist" / f"harmos-{platform()}.zip"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
        for path in sorted(root.rglob("*")):
            if path.is_file():
                output.write(path, path.relative_to(root).as_posix())


def validate(repository: Path) -> None:
    (location(repository) / "reports/validation.json").unlink(missing_ok=True)
    verify(repository)
    directory = repository / ".stkr/tests"
    manifest = json.loads((directory / "latest.json").read_text(encoding="utf-8"))
    policy_document = tomllib.loads(
        (repository / "tools/python/evidence.toml").read_text(encoding="utf-8")
    )
    policy = policy_document["testing"]
    selection_policy = evidence.parse_policy(policy_document)
    build = json.loads(
        (location(repository) / "reports/build.json").read_text(encoding="utf-8")
    )
    revision = _output(repository, ["git", "rev-parse", "HEAD"])
    if os.environ.get("CI") == "true" and (
        build.get("run") != os.environ.get("GITHUB_RUN_ID")
        or build.get("job") != f"build_release_{platform()}"
        or not re.fullmatch(r"[1-9][0-9]*", str(build.get("attempt", "")))
    ):
        raise RuntimeError("release build provenance does not match the validation run")
    expected = {
        f"{category}-{language}"
        for language in ("rust", "python")
        for category in policy[language]["categories"]
    }
    records = manifest.get("tests", {})
    if set(records) != expected:
        raise RuntimeError("required test categories are incomplete")
    for name in expected:
        entry = records[name]
        if entry.get("outcome") != "passed" or entry.get("requested_configuration"):
            raise RuntimeError(f"required full test category {name} did not pass")
        category, language = name.split("-")
        if (
            entry.get("category") != category
            or entry.get("language") != language
            or entry.get("revision") != revision
            or entry.get("platform") != platform()
        ):
            raise RuntimeError(
                f"required test category {name} has stale or mismatched provenance"
            )
        started = entry.get("started_unix_seconds", 0)
        finished = entry.get("finished_unix_seconds", 0)
        if (
            not build.get("built_unix_seconds")
            or started < build["built_unix_seconds"]
            or finished < started
        ):
            raise RuntimeError(
                f"required test category {name} predates its release package"
            )
        configurations = entry.get("configurations", [])
        expected_configurations = {
            item.name
            for item in evidence.selections(selection_policy, category, language)
        }
        if (
            len(configurations) != len(expected_configurations)
            or {item.get("configuration") for item in configurations}
            != expected_configurations
            or any(item.get("success") is not True for item in configurations)
        ):
            raise RuntimeError(
                f"required test category {name} has incomplete configurations"
            )
        report = directory / f"tests-{name}.xml"
        summary = evidence.summarize(
            report.read_text(encoding="utf-8"), entry["category"], entry["coverage"]
        )
        if summary["failed"]:
            raise RuntimeError(f"required category {name} contains failures")
        if os.environ.get("CI") == "true" and not entry.get("coverage"):
            raise RuntimeError(f"required CI coverage is absent for {name}")
    if os.environ.get("CI") == "true":
        for language in ("rust", "python"):
            evidence.validate_coverage(
                (directory / f"coverage-{language}.xml").read_text(encoding="utf-8")
            )
    root = location(repository)
    record = {
        "schema": 2,
        "platform": platform(),
        "source": source(repository),
        "run": os.environ.get("GITHUB_RUN_ID"),
        "attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
        "job": os.environ.get("GITHUB_JOB"),
        "build_manifest": _digest(root / "reports/build.json"),
        "evidence": _digest(directory / "latest.json"),
    }
    (root / "reports/validation.json").write_text(
        json.dumps(record, indent=2) + "\n", encoding="utf-8"
    )


def reuse(repository: Path) -> None:
    # One Native Run Identifies Both Required Platform Packages
    run_id = os.environ.get("HARMOS_VALIDATION_RUN_ID") or _reference(repository)
    if not re.fullmatch(r"[1-9][0-9]*", run_id):
        raise RuntimeError(
            "set HARMOS_VALIDATION_RUN_ID to the original full validation run"
        )
    run = _json(f"actions/runs/{run_id}")
    if run.get("conclusion") != "success" or run.get("status") != "completed":
        raise RuntimeError("the original validation run has not succeeded")
    if run.get("path") != ".github/workflows/full.yml":
        raise RuntimeError(
            "the original run is not the required full validation workflow"
        )
    revision = run.get("head_sha", "")
    if not re.fullmatch(r"[0-9a-f]{40,64}", revision):
        raise RuntimeError("the original run commit is invalid")
    subprocess.run(
        ["git", "fetch", "--no-tags", "origin", revision],
        cwd=repository,
        check=True,
        env=_git_environment(),
    )
    if _output(repository, ["git", "rev-parse", f"{revision}^{{tree}}"]) != _output(
        repository, ["git", "rev-parse", "HEAD^{tree}"]
    ):
        raise RuntimeError(
            "the original validation run source tree differs from the proposed integration"
        )
    jobs = {}
    attempt_count = run.get("run_attempt", 0)
    if not isinstance(attempt_count, int) or not 1 <= attempt_count <= 100:
        raise RuntimeError("invalid native validation run attempts")
    for attempt in range(1, attempt_count + 1):
        for job in _pages(f"actions/runs/{run_id}/attempts/{attempt}/jobs", "jobs"):
            if job["id"] not in jobs:
                jobs[job["id"]] = job | {"attempt": attempt}

    required = {
        f"{phase} ({system})"
        for phase in ("check", "Build Release", "test")
        for system in ("linux", "windows")
    }
    selected = {}
    for job in jobs.values():
        name = job.get("name")
        if name in required and job["id"] > selected.get(name, {}).get("id", 0):
            selected[name] = job
    if set(selected) != required or any(
        job.get("conclusion") != "success" for job in selected.values()
    ):
        raise RuntimeError(
            "the original run has not completed the required native platform matrix"
        )
    job = selected[f"test ({platform()})"]
    artifact_name = f"harmos-test-{platform()}-{job['attempt']}"
    artifacts = _pages(f"actions/runs/{run_id}/artifacts", "artifacts")
    matches = [
        item
        for item in artifacts
        if item.get("name") == artifact_name and not item.get("expired")
    ]
    if len(matches) != 1:
        raise RuntimeError(
            "validated platform artifacts are missing or expired; no rebuild was attempted"
        )
    artifact = matches[0]
    if artifact.get("workflow_run", {}).get("head_sha") != revision or artifact.get(
        "workflow_run", {}
    ).get("id") != int(run_id):
        raise RuntimeError("artifact does not belong to the original validation run")
    endpoint = f"actions/artifacts/{artifact['id']}/zip"
    root = location(repository)
    temporary_root = repository / ".tmp"
    temporary_root.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="package-reuse-", dir=temporary_root
    ) as temporary:
        archive = Path(temporary) / "artifacts.zip"
        try:
            with (
                urlopen(_request(endpoint), timeout=60) as response,
                archive.open("wb") as output,
            ):
                shutil.copyfileobj(response, output)
        except HTTPError as error:
            raise RuntimeError(
                "validated platform artifacts are missing, expired, or inaccessible; no rebuild was attempted"
            ) from error
        if artifact.get("digest") != f"sha256:{_digest(archive)}":
            raise RuntimeError("original native artifact digest is corrupt")
        extracted = Path(temporary) / "package"
        prefix = f"{platform()}/"
        with zipfile.ZipFile(archive) as files:
            _check_archive(files)
            download = Path(temporary) / "download.zip"
            with (
                files.open(f"harmos-{platform()}.zip") as content,
                download.open("wb") as output,
            ):
                shutil.copyfileobj(content, output)
            for member in files.infolist():
                if not member.filename.startswith(prefix) or member.is_dir():
                    continue
                relative = Path(member.filename.removeprefix(prefix))
                if relative.is_absolute() or ".." in relative.parts:
                    raise RuntimeError("unsafe platform artifact path")
                destination = extracted / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                with files.open(member) as content, destination.open("wb") as output:
                    shutil.copyfileobj(content, output)
                destination.chmod((member.external_attr >> 16) & 0o755 or 0o644)
        record = json.loads(
            (extracted / "reports/validation.json").read_text(encoding="utf-8")
        )
        if (
            record.get("schema") != 2
            or record.get("platform") != platform()
            or record.get("run") != run_id
            or record.get("job") != f"test_{platform()}"
            or record.get("attempt") != str(job["attempt"])
            or record.get("source") != source(repository)
        ):
            raise RuntimeError(
                "original validation does not match the proposed source, platform, or run"
            )
        if record.get("build_manifest") != _digest(
            extracted / "reports/build.json"
        ) or record.get("evidence") != _digest(extracted / "tests/latest.json"):
            raise RuntimeError("original validation artifact metadata is corrupt")
        build = json.loads(
            (extracted / "reports/build.json").read_text(encoding="utf-8")
        )
        build_job = selected[f"Build Release ({platform()})"]
        if (
            build.get("run") != run_id
            or build.get("job") != f"build_release_{platform()}"
            or build.get("attempt") != str(build_job["attempt"])
            or build.get("revision") != revision
        ):
            raise RuntimeError("original build does not match the required native job")
        paths = _verify(repository, extracted)
        if os.name != "nt":
            for name in BINARIES:
                paths[name].chmod(0o755)
        with zipfile.ZipFile(download) as files:
            _check_archive(files)
            expected = {
                path.relative_to(extracted).as_posix(): path
                for path in extracted.rglob("*")
                if path.is_file()
            }
            members = {
                member.filename for member in files.infolist() if not member.is_dir()
            }
            if members != set(expected):
                raise RuntimeError("the original downloadable archive is incomplete")
            for relative, path in expected.items():
                with files.open(relative) as content:
                    digest = hashlib.sha256()
                    for chunk in iter(lambda: content.read(262144), b""):
                        digest.update(chunk)
                    if digest.hexdigest() != _digest(path):
                        raise RuntimeError(
                            f"the original downloadable archive differs at {relative}"
                        )
        if root.exists():
            shutil.rmtree(root)
        root.parent.mkdir(exist_ok=True)
        shutil.move(str(extracted), root)
        shutil.move(str(download), root.parent / f"harmos-{platform()}.zip")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "action", choices=("stage", "verify", "restore", "validate", "export", "reuse")
    )
    arguments = parser.parse_args()
    repository = Path(__file__).resolve().parents[4]
    {
        "stage": stage,
        "verify": verify,
        "restore": restore,
        "validate": validate,
        "export": export,
        "reuse": reuse,
    }[arguments.action](repository)


# === Private Functions === #


def _verify(repository: Path, root: Path) -> dict[str, Path]:
    manifest = json.loads((root / "reports/build.json").read_text(encoding="utf-8"))
    if manifest.get("schema") != 2 or manifest.get("platform") != platform():
        raise RuntimeError("release package schema or platform does not match")
    if manifest.get("source") != source(repository):
        raise RuntimeError(
            "release package source inputs do not match the current proposed tree"
        )
    if manifest.get("inputs") != release.inputs(repository):
        raise RuntimeError(
            "release package resolved inputs do not match the proposed integration"
        )
    for name, command in VERSION_COMMANDS.items():
        if manifest.get("versions", {}).get(name) != _output(repository, command):
            raise RuntimeError(
                f"recorded {name} inputs do not match the resolved toolchain"
            )
    license_file = root / "LICENSE-APACHE"
    if (
        not license_file.is_file()
        or license_file.is_symlink()
        or _digest(license_file) != _digest(repository / "LICENSE-APACHE")
    ):
        raise RuntimeError("release package Apache license is missing or corrupt")
    suffix = ".exe" if os.name == "nt" else ""
    expected = {name: f"cli/{name}{suffix}" for name in BINARIES} | PLUGINS
    if manifest.get("artifacts") != expected or set(manifest.get("digests", {})) != set(
        expected.values()
    ):
        raise RuntimeError("release package manifest is incomplete")
    paths = {}
    for name, relative in expected.items():
        path = root / relative
        if (
            not path.is_file()
            or path.is_symlink()
            or _digest(path) != manifest["digests"][relative]
        ):
            raise RuntimeError(f"release artifact {name} is missing or corrupt")
        paths[name] = path
    return paths


def _check_archive(archive: zipfile.ZipFile) -> None:
    names = set()
    for member in archive.infolist():
        path = PurePosixPath(member.filename)
        if (
            path.is_absolute()
            or ".." in path.parts
            or "\\" in member.filename
            or ":" in member.filename
            or stat.S_ISLNK(member.external_attr >> 16)
            or member.filename.casefold() in names
        ):
            raise RuntimeError("unsafe or duplicate platform artifact path")
        names.add(member.filename.casefold())


def _reference(repository: Path) -> str:
    revision = _json("branches/dev")["commit"]["sha"]
    tree = _json(f"git/commits/{revision}")["tree"]["sha"]
    if tree != _output(repository, ["git", "rev-parse", "HEAD^{tree}"]):
        raise RuntimeError("published dev source tree differs from proposed promotion")
    # Native Squash Association Carries Original Provenance Across Integration
    requests = _pages(f"commits/{revision}/pulls", None)
    matches = [
        item
        for item in requests
        if item.get("merged_at")
        and item.get("base", {}).get("ref") == "dev"
        and item.get("merge_commit_sha") == revision
    ]
    if not matches:
        baseline = os.environ.get("HARMOS_BASELINE_VALIDATION_RUN_ID")
        if baseline:
            return baseline
    if len(matches) != 1:
        raise RuntimeError(
            "no unique native dev integration reference; supply HARMOS_VALIDATION_RUN_ID explicitly"
        )
    head = matches[0]["head"]["sha"]
    runs = _pages(
        f"actions/workflows/full.yml/runs?head_sha={head}&status=success",
        "workflow_runs",
    )
    if len(runs) != 1:
        raise RuntimeError(
            "no unique original full validation run; supply HARMOS_VALIDATION_RUN_ID explicitly"
        )
    return str(runs[0]["id"])


def _request(endpoint: str) -> Request:
    request = Request(
        f"https://api.github.com/repos/stokker-technologies/harmos/{endpoint}"
    )
    request.add_header("Accept", "application/vnd.github+json")
    request.add_header("X-GitHub-Api-Version", "2022-11-28")
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        # Never Forward Native Credentials To Artifact Storage Redirects
        request.add_unredirected_header("Authorization", f"Bearer {token}")
    return request


def _json(endpoint: str):
    with urlopen(_request(endpoint), timeout=30) as response:
        return json.load(response)


def _pages(endpoint: str, field: str | None) -> list[dict]:
    result = []
    separator = "&" if "?" in endpoint else "?"
    for page in range(1, 101):
        document = _json(f"{endpoint}{separator}per_page=100&page={page}")
        values = document[field] if field else document
        result.extend(values)
        if len(values) < 100:
            return result
    raise RuntimeError("native artifact lookup exceeded pagination limit")


def _git_environment() -> dict[str, str]:
    environment = os.environ.copy()
    for key in (
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ):
        environment.pop(key, None)
    return environment


def _target(repository: Path) -> Path:
    metadata = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version=1"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
    )
    return Path(json.loads(metadata.stdout)["target_directory"])


def _digest(path: Path) -> str:
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def _output(repository: Path, command: list[str]) -> str:
    return subprocess.run(
        command,
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
        env=_git_environment(),
    ).stdout.strip()


# === Entry Point === #

if __name__ == "__main__":
    main()
