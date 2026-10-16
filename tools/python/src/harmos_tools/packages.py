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
import gzip
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tarfile
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
    "cargo-harmos",
    "schematic-cli",
    "schematic-sidecar",
    "harmos-lifecycle-fixture",
    "harmos-lifecycle-sidecar",
    "harmos-observer-fixture",
)
PLUGINS = {"schematic-tools": "examples/schematic/plugins/schematic-guest.wasm"}

SCHEMA_PRODUCERS = {"schematic-cli", "schematic-sidecar", "schematic-guest"}

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


def build_profile() -> str:
    profile = os.environ.get("STKR_TEST_PROFILE", "dev")
    if profile not in {"dev", "release"}:
        raise ValueError("STKR_TEST_PROFILE must be dev or release")
    return profile


def build(repository: Path) -> None:
    target = _target(repository)
    metadata = json.loads(
        _output(
            repository,
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version=1"],
        )
    )
    producers = {
        package["id"]: package["name"]
        for package in metadata["packages"]
        if package["name"] in SCHEMA_PRODUCERS
    }
    outputs = {}
    artifacts = {}
    index = target / "harmos-build/schemas.json"
    index.unlink(missing_ok=True)
    versions = _tool_versions(repository)
    host = next(
        line.removeprefix("host: ")
        for line in versions["rust"].splitlines()
        if line.startswith("host: ")
    )
    if host not in {"x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"}:
        raise RuntimeError(f"undeclared native release target: {host}")
    commands = [
        [
            "cargo",
            "build",
            "--locked",
            *(["--release"] if build_profile() == "release" else []),
            "--timings",
            "--message-format=json-render-diagnostics",
            "-p",
            "cargo-harmos",
            "-p",
            "schematic-cli",
            "-p",
            "schematic-sidecar",
            "-p",
            "harmos-lifecycle-fixture",
            "-p",
            "harmos-observer-fixture",
            "--target",
            host,
        ],
    ]
    for command in commands:
        result = subprocess.run(
            command, cwd=repository, check=False, capture_output=True, text=True
        )
        sys.stderr.write(result.stderr)
        for line in result.stdout.splitlines():
            message = json.loads(line)
            rendered = message.get("message", {}).get("rendered")
            if rendered:
                sys.stderr.write(rendered)
            if message.get("reason") == "compiler-artifact":
                name = message["target"]["name"]
                executable = message.get("executable")
                if name in BINARIES and executable:
                    artifacts[name] = executable
            producer = producers.get(message.get("package_id"))
            if message.get("reason") == "build-script-executed" and producer:
                directory = Path(message["out_dir"])
                if not directory.is_absolute():
                    raise RuntimeError("Cargo schema output must be absolute")
                previous = outputs.setdefault(producer, str(directory))
                if previous != str(directory):
                    raise RuntimeError(
                        f"multiple schema build contexts selected for {producer}"
                    )
        result.check_returncode()
    # Keep Development Assertions With Basic Guest Optimization
    guest_environment = os.environ.copy()
    if build_profile() == "dev":
        guest_environment["CARGO_PROFILE_DEV_OPT_LEVEL"] = "1"
    # The Reusable Guest Build Owns Compilation And Componentization Together.
    result = subprocess.run(
        [
            artifacts["cargo-harmos"],
            "guest",
            "build",
            "-p",
            "schematic-guest",
            *(["--release"] if build_profile() == "release" else []),
            "--timings",
            "--json",
        ],
        cwd=repository,
        env=guest_environment,
        check=False,
        capture_output=True,
        text=True,
    )
    sys.stderr.write(result.stderr)
    result.check_returncode()
    prepared = json.loads(result.stdout)
    guests = [
        artifact
        for artifact in prepared["artifacts"]
        if artifact["producer"] == "schematic-guest"
    ]
    schemas = [
        schema
        for schema in prepared["schemas"]
        if schema["producer"] == "schematic-guest" and schema["catalog"] == "operations"
    ]
    if len(guests) != 1 or len(schemas) != 1:
        raise RuntimeError("guest build did not select its exact artifact and schema")
    artifacts["schematic_guest"] = guests[0]["path"]
    outputs["schematic-guest"] = str(Path(schemas[0]["directory"]).parent.parent)
    if set(artifacts) != set(BINARIES) | {"schematic_guest"}:
        raise RuntimeError(
            "selected build did not report every release and acceptance artifact"
        )
    if set(outputs) != SCHEMA_PRODUCERS:
        raise RuntimeError(
            "selected build did not report every schematic schema producer"
        )
    index.parent.mkdir(parents=True, exist_ok=True)
    index.write_text(
        json.dumps(
            {
                "profile": build_profile(),
                "revision": _output(repository, ["git", "rev-parse", "HEAD"]),
                "source": source(repository),
                "inputs": release.inputs(repository),
                "versions": versions,
                "target": host,
                "artifacts": artifacts,
                "digests": {
                    name: _digest(Path(filename))
                    for name, filename in artifacts.items()
                },
                "schema_digests": {
                    producer: {
                        name: _digest(Path(directory) / "grpc/operations" / name)
                        for name in _schema_files(Path(directory) / "grpc/operations")
                    }
                    for producer, directory in outputs.items()
                },
                "outputs": outputs,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )


def stage(repository: Path) -> None:
    inputs = release.inputs(repository)
    if os.environ.get("CI") == "true" and not inputs["source_tree"]:
        raise RuntimeError("CI release packaging requires clean committed source")
    target = _target(repository)
    schema_index = _build_index(repository, target)
    root = location(repository)
    for filename in (f"harmos-ci-{platform()}.zip", f"harmos-tests-{platform()}.zip"):
        (root.parent / filename).unlink(missing_ok=True)
    if root.exists():
        shutil.rmtree(root)
    (root / "cli").mkdir(parents=True)
    (root / "reports").mkdir()
    shutil.copy2(repository / "LICENSE-APACHE", root / "LICENSE-APACHE")
    artifacts = artifact_paths()
    for name in BINARIES:
        destination = root / artifacts[name]
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(schema_index["artifacts"][name], destination)
    guest = root / PLUGINS["schematic-tools"]
    guest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(schema_index["artifacts"]["schematic_guest"], guest)
    artifacts.update(PLUGINS)
    schemas = {}
    for producer, output in sorted(schema_index["outputs"].items()):
        directory = Path(output) / "grpc/operations"
        for name in _schema_files(directory):
            relative = f"examples/schematic/grpc/{producer}/operations/{name}"
            destination = root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(directory / name, destination)
            schemas[relative] = _digest(destination)
    # Build Provenance Binds The Exact Commit SHA
    revision = _output(repository, ["git", "rev-parse", "HEAD"])
    versions = schema_index["versions"]
    manifest = {
        "schema": 2,
        "profile": build_profile(),
        "platform": platform(),
        "target": schema_index["target"],
        "version": tomllib.loads(
            (repository / "Cargo.toml").read_text(encoding="utf-8")
        )["workspace"]["package"]["version"],
        "built_unix_seconds": int(time.time()),
        "run": os.environ.get("GITHUB_RUN_ID"),
        "attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
        "job": os.environ.get("GITHUB_JOB"),
        "revision": revision,
        "source": source(repository),
        "inputs": inputs,
        "versions": versions,
        "artifacts": artifacts,
        "schemas": schemas,
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
    archive_payloads(repository, root)
    restore(repository)
    export(repository)


def verify(repository: Path) -> dict[str, Path]:
    return _verify(repository, location(repository))


def restore(repository: Path) -> None:
    root = location(repository)
    if root.is_symlink() or any(
        parent.is_symlink()
        for parent in root.parents
        if parent.is_relative_to(repository)
    ):
        raise RuntimeError("package restoration refuses symlink destinations")
    if root.exists() and (
        not root.is_dir() or any(path.is_symlink() for path in root.rglob("*"))
    ):
        raise RuntimeError(
            "package restoration requires regular files without symlinks"
        )
    envelope = root.parent / f"harmos-ci-{platform()}.zip"
    root.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="package-restore-", dir=root.parent
    ) as temporary:
        staging = Path(temporary) / "incoming"
        if (root / "reports/build.json").is_file():
            shutil.copytree(root, staging, symlinks=True)
        elif envelope.is_file() and not envelope.is_symlink():
            staging.mkdir()
            with zipfile.ZipFile(envelope) as archive:
                _check_archive(archive)
                prefix = platform() + "/"
                for member in archive.infolist():
                    if member.is_dir():
                        continue
                    if not member.filename.startswith(prefix):
                        raise RuntimeError(
                            "CI envelope contains an unexpected native platform"
                        )
                    destination = staging / member.filename.removeprefix(prefix)
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    with (
                        archive.open(member) as content,
                        destination.open("wb") as output,
                    ):
                        shutil.copyfileobj(content, output)
        else:
            raise FileNotFoundError(f"release package {root} is absent")
        manifest = json.loads(
            (staging / "reports/build.json").read_text(encoding="utf-8")
        )
        _consume_containers(staging, manifest, extract=True)
        paths = _verify(repository, staging)
        # Native Actions Transfer Preserves Bytes But Drops Executable Permissions.
        if os.name != "nt":
            for name in BINARIES:
                paths[name].chmod(0o755)
        # Install Only Verified Bytes; Restore The Previous Directory On Failure.
        previous = Path(temporary) / "previous"
        try:
            if root.exists():
                root.rename(previous)
            staging.rename(root)
        except BaseException:
            if previous.exists():
                if root.exists():
                    root.rename(staging)
                previous.rename(root)
            raise


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
    build = json.loads((root / "reports/build.json").read_text(encoding="utf-8"))
    contained = {
        name
        for container in build["containers"].values()
        for name in container["members"]
    }
    # Build Payloads Are Transferred Once; Later Tests Export Only Their Reports.
    archive = repository / "dist" / f"harmos-ci-{platform()}.zip"
    if not archive.exists():
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
            for path in sorted(root.rglob("*")):
                relative = path.relative_to(root).as_posix()
                if (
                    not path.is_file()
                    or relative in contained
                    or relative.startswith(("tests/", "reports/test-diagnostics/"))
                    or relative == "reports/validation.json"
                ):
                    continue
                compression = (
                    zipfile.ZIP_STORED
                    if relative.startswith("archives/")
                    else zipfile.ZIP_DEFLATED
                )
                output.write(
                    path, f"{platform()}/{relative}", compress_type=compression
                )
    if (root / "tests").is_dir() or (root / "reports/validation.json").is_file():
        archive = repository / "dist" / f"harmos-tests-{platform()}.zip"
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
            for directory in (root / "tests", root / "reports"):
                for path in sorted(directory.rglob("*")):
                    relative = path.relative_to(root).as_posix()
                    if path.is_file() and (
                        relative.startswith(("tests/", "reports/test-diagnostics/"))
                        or relative in {"reports/build.json", "reports/validation.json"}
                    ):
                        output.write(path, relative)


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
        or build.get("job") != f"build_ci_{platform()}"
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
    measured = set()
    requested_coverage = os.environ.get("STKR_COVERAGE") == "1"
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
        if requested_coverage and not entry.get("coverage"):
            raise RuntimeError(f"requested coverage is absent for {name}")
        if entry.get("coverage"):
            measured.add(language)
    for language in measured:
        evidence.validate_coverage(
            (directory / f"coverage-{language}.xml").read_text(encoding="utf-8")
        )
    root = location(repository)
    record = {
        "schema": 2,
        "profile": build_profile(),
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


def reuse(repository: Path, *, development: bool = False) -> dict:
    # One Native Run Identifies Required Platform Packages
    run_id = os.environ.get("HARMOS_VALIDATION_RUN_ID") or _reference(repository)
    if not re.fullmatch(r"[1-9][0-9]*", run_id):
        raise RuntimeError(
            "set HARMOS_VALIDATION_RUN_ID to the original full validation run"
        )
    run = _json(f"actions/runs/{run_id}")
    if development:
        _development()
        if str(run.get("id")) != os.environ.get("GITHUB_RUN_ID"):
            raise RuntimeError("development proof requires the current native run")
        if run.get("event") != os.environ.get("GITHUB_EVENT_NAME"):
            raise RuntimeError("development proof event differs from the native run")
    elif run.get("conclusion") != "success" or run.get("status") != "completed":
        raise RuntimeError("the original validation run has not succeeded")
    if run.get("path") != ".github/workflows/full.yml":
        raise RuntimeError(
            "the original run is not the required full validation workflow"
        )
    revision = run.get("head_sha", "")
    if not re.fullmatch(r"[0-9a-f]{40,64}", revision):
        raise RuntimeError("the original run commit is invalid")
    if revision != _output(repository, ["git", "rev-parse", "HEAD"]):
        raise RuntimeError("a new commit SHA requires a fresh build and validation")
    jobs = {}
    attempt_count = run.get("run_attempt", 0)
    if not isinstance(attempt_count, int) or not 1 <= attempt_count <= 100:
        raise RuntimeError("invalid native validation run attempts")
    for attempt in range(1, attempt_count + 1):
        for job in _pages(f"actions/runs/{run_id}/attempts/{attempt}/jobs", "jobs"):
            if job["id"] not in jobs:
                jobs[job["id"]] = job | {"attempt": attempt}

    required = {
        label
        for system in (("linux",) if development else ("linux", "windows"))
        for label in (
            f"Check Fast ({system.title()})",
            f"Build CI ({system.title()})",
            f"test ({system})",
        )
    }
    if development:
        required.add("check:merge-route")
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
    artifacts = _pages(f"actions/runs/{run_id}/artifacts", "artifacts")
    build_job = selected[f"Build CI ({platform().title()})"]
    root = location(repository)
    temporary_root = repository / ".tmp"
    temporary_root.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="package-reuse-", dir=temporary_root
    ) as temporary:
        extracted = Path(temporary) / "package"
        downloads = {}
        proven_artifacts = []
        for phase, original_job, filename in [
            ("build", build_job, f"harmos-ci-{platform()}.zip"),
            ("test", job, f"harmos-tests-{platform()}.zip"),
        ]:
            artifact_name = f"harmos-{phase}-{platform()}-{original_job['attempt']}"
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
            if artifact.get("workflow_run", {}).get(
                "head_sha"
            ) != revision or artifact.get("workflow_run", {}).get("id") != int(run_id):
                raise RuntimeError(
                    "artifact does not belong to the original validation run"
                )
            outer = Path(temporary) / f"{phase}-artifact.zip"
            try:
                with (
                    urlopen(
                        _request(f"actions/artifacts/{artifact['id']}/zip"), timeout=60
                    ) as response,
                    outer.open("wb") as output,
                ):
                    shutil.copyfileobj(response, output)
            except HTTPError as error:
                raise RuntimeError(
                    "validated platform artifacts are missing, expired, or inaccessible; no rebuild was attempted"
                ) from error
            if artifact.get("digest") != f"sha256:{_digest(outer)}":
                raise RuntimeError("original native artifact digest is corrupt")
            download = Path(temporary) / filename
            with zipfile.ZipFile(outer) as files:
                _check_archive(files)
                if files.namelist() != [filename]:
                    raise RuntimeError("native artifact contains unexpected payloads")
                with files.open(filename) as content, download.open("wb") as output:
                    shutil.copyfileobj(content, output)
            downloads[filename] = download
            proven_artifacts.append(
                {
                    "id": artifact["id"],
                    "name": artifact_name,
                    "provider_digest": artifact["digest"],
                    "payload": filename,
                    "sha256": _digest(download),
                    "job": original_job["id"],
                    "attempt": original_job["attempt"],
                }
            )
            with zipfile.ZipFile(download) as files:
                _check_archive(files)
                for member in files.infolist():
                    if member.is_dir():
                        continue
                    relative = member.filename
                    if phase == "build":
                        prefix = f"{platform()}/"
                        if not relative.startswith(prefix):
                            raise RuntimeError(
                                "CI envelope contains an unexpected native platform"
                            )
                        relative = relative.removeprefix(prefix)
                    elif not relative.startswith(("reports/", "tests/")):
                        raise RuntimeError("test archive contains unexpected payloads")
                    destination = extracted / relative
                    if destination.exists():
                        with files.open(member) as content:
                            digest = hashlib.sha256()
                            for chunk in iter(lambda: content.read(262144), b""):
                                digest.update(chunk)
                            if digest.hexdigest() != _digest(destination):
                                raise RuntimeError(
                                    "test provenance differs from the original build"
                                )
                        continue
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    with (
                        files.open(member) as content,
                        destination.open("wb") as output,
                    ):
                        shutil.copyfileobj(content, output)
        _consume_containers(
            extracted,
            json.loads((extracted / "reports/build.json").read_text(encoding="utf-8")),
            extract=True,
        )
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
        build_job = selected[f"Build CI ({platform().title()})"]
        if (
            build.get("run") != run_id
            or build.get("job") != f"build_ci_{platform()}"
            or build.get("attempt") != str(build_job["attempt"])
            or build.get("revision") != revision
        ):
            raise RuntimeError("original build does not match the required native job")
        paths = _verify(repository, extracted)
        if os.name != "nt":
            for name in BINARIES:
                paths[name].chmod(0o755)
        if root.exists():
            shutil.rmtree(root)
        root.parent.mkdir(exist_ok=True)
        shutil.move(str(extracted), root)
        for filename, download in downloads.items():
            shutil.move(str(download), root.parent / filename)
    return {
        "revision": revision,
        "source": source(repository),
        "run": int(run_id),
        "jobs": {name: job["id"] for name, job in selected.items()},
        "artifacts": proven_artifacts,
        "validation": record,
    }


def development_catalog(repository: Path) -> None:
    destination = repository / "dist/development.json"
    destination.unlink(missing_ok=True)
    _development()
    proof = reuse(repository, development=True)
    catalog = {
        "schema": 1,
        "purpose": "development-validation",
        "platforms": ["linux"],
        "proof": proof,
    }
    destination.write_text(json.dumps(catalog, indent=2) + "\n", encoding="utf-8")


def development_fast() -> bool:
    event = os.environ.get("GITHUB_EVENT_NAME")
    target = (
        os.environ.get("GITHUB_BASE_REF")
        if event == "pull_request"
        else os.environ.get("GITHUB_REF")
    )
    return (event, target) in {
        ("pull_request", "dev"),
        ("push", "refs/heads/dev"),
    }


def development_policy() -> None:
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"fast={str(development_fast()).lower()}\n")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "action",
        choices=(
            "build",
            "stage",
            "verify",
            "restore",
            "validate",
            "export",
            "reuse",
            "development",
            "policy",
        ),
    )
    arguments = parser.parse_args()
    repository = Path(__file__).resolve().parents[4]
    {
        "build": build,
        "stage": stage,
        "verify": verify,
        "restore": restore,
        "validate": validate,
        "export": export,
        "reuse": reuse,
        "development": development_catalog,
        "policy": lambda _: development_policy(),
    }[arguments.action](repository)


# === Private Functions === #


def _development() -> None:
    if not development_fast() or platform() != "linux":
        raise RuntimeError("Linux-only proof is restricted to native dev validation")


def _verify(repository: Path, root: Path) -> dict[str, Path]:
    manifest = json.loads((root / "reports/build.json").read_text(encoding="utf-8"))
    if manifest.get("profile") != build_profile():
        raise RuntimeError(
            "artifact profile differs from the selected validation profile"
        )
    if manifest.get("schema") != 2 or manifest.get("platform") != platform():
        raise RuntimeError("release package schema or platform does not match")
    if manifest.get("revision") != _output(repository, ["git", "rev-parse", "HEAD"]):
        raise RuntimeError("a new commit SHA requires a fresh build and validation")
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
    expected = artifact_paths()
    _consume_containers(root, manifest, extract=False)
    if manifest.get("artifacts") != expected or set(manifest.get("digests", {})) != set(
        expected.values()
    ):
        raise RuntimeError("release package manifest is incomplete")
    schemas = manifest.get("schemas", {})
    if not isinstance(schemas, dict):
        raise RuntimeError("schema build provenance is invalid")
    for producer in SCHEMA_PRODUCERS:
        directory = root / f"examples/schematic/grpc/{producer}/operations"
        names = _schema_files(directory)
        prefix = directory.relative_to(root).as_posix() + "/"
        declared = {prefix + name for name in names}
        if {name for name in schemas if name.startswith(prefix)} != declared:
            raise RuntimeError("schema inventory differs from its generated manifest")
        for name in declared:
            if _digest(root / name) != schemas[name]:
                raise RuntimeError(f"generated schema is corrupt: {name}")
    if any(
        not any(
            name.startswith(f"examples/schematic/grpc/{producer}/operations/")
            for producer in SCHEMA_PRODUCERS
        )
        for name in schemas
    ):
        raise RuntimeError("schema contains an unexpected producer or catalog")
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


def artifact_paths() -> dict[str, str]:
    suffix = ".exe" if os.name == "nt" else ""
    paths = {name: f"fixtures/{name}{suffix}" for name in BINARIES}
    paths["cargo-harmos"] = f"cli/cargo-harmos{suffix}"
    paths["schematic-cli"] = f"examples/schematic/schematic-cli{suffix}"
    paths["schematic-sidecar"] = f"examples/schematic/plugins/schematic-sidecar{suffix}"
    return paths | PLUGINS


def archive_payloads(repository: Path, root: Path) -> None:
    report = root / "reports/build.json"
    build = json.loads(report.read_text(encoding="utf-8"))
    version = build["version"]
    target = build["target"]
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) or target not in {
        "x86_64-unknown-linux-gnu",
        "x86_64-pc-windows-msvc",
    }:
        raise RuntimeError(
            "tool archive requires a stable version and declared native target"
        )
    suffix = ".exe" if target.endswith("windows-msvc") else ""
    tool_root = f"cargo-harmos-{version}-{target}"
    extension = ".zip" if suffix else ".tar.gz"
    tool_path = f"archives/{tool_root}{extension}"
    example_path = f"archives/harmos-examples-{target}.zip"
    shutil.copy2(repository / "README.md", root / "cli/README.md")
    shutil.copy2(repository / "LICENSE-APACHE", root / "cli/LICENSE-APACHE")
    for source, destination in [
        (repository / "README.md", root / "README.md"),
        (
            repository / "examples/schematic/README.md",
            root / "examples/schematic/README.md",
        ),
        (report, root / "examples/build.json"),
    ]:
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    tool = {
        f"cli/cargo-harmos{suffix}": f"{tool_root}/cargo-harmos{suffix}",
        "cli/README.md": f"{tool_root}/README.md",
        "cli/LICENSE-APACHE": f"{tool_root}/LICENSE-APACHE",
    }
    examples = {
        path.relative_to(root).as_posix(): path.relative_to(root).as_posix()
        for path in (root / "examples/schematic").rglob("*")
        if path.is_file()
    }
    examples.update(
        {
            "README.md": "README.md",
            "LICENSE-APACHE": "LICENSE-APACHE",
            "examples/build.json": "reports/build.json",
        }
    )
    containers = {}
    for name, relative, members in [
        ("tool", tool_path, tool),
        ("examples", example_path, examples),
    ]:
        archive = root / relative
        archive.parent.mkdir(parents=True, exist_ok=True)
        if relative.endswith(".zip"):
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
                for source, member in sorted(members.items()):
                    output.write(root / source, member)
        else:
            with (
                archive.open("wb") as file,
                gzip.GzipFile(
                    fileobj=file, mode="wb", filename="", mtime=0
                ) as compressed,
                tarfile.open(fileobj=compressed, mode="w") as output,
            ):
                for source, member in sorted(members.items()):
                    info = output.gettarinfo(root / source, arcname=member)
                    info.uid = info.gid = 0
                    info.uname = info.gname = ""
                    info.mtime = 0
                    with (root / source).open("rb") as content:
                        output.addfile(info, content)
        containers[name] = {
            "path": relative,
            "sha256": _digest(archive),
            "members": members,
            "digests": {source: _digest(root / source) for source in members},
        }
    build["containers"] = containers
    report.write_text(json.dumps(build, indent=2) + "\n", encoding="utf-8")


def _consume_containers(root: Path, build: dict, *, extract: bool) -> None:
    containers = build.get("containers", {})
    if set(containers) != {"tool", "examples"}:
        raise RuntimeError("tool and example archive inventory is incomplete")
    version, target = build.get("version", ""), build.get("target", "")
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) or target not in {
        "x86_64-unknown-linux-gnu",
        "x86_64-pc-windows-msvc",
    }:
        raise RuntimeError("invalid tool archive version or target")
    suffix = ".exe" if target.endswith("windows-msvc") else ""
    tool_root = f"cargo-harmos-{version}-{target}"
    tool = {
        f"cli/cargo-harmos{suffix}": f"{tool_root}/cargo-harmos{suffix}",
        "cli/README.md": f"{tool_root}/README.md",
        "cli/LICENSE-APACHE": f"{tool_root}/LICENSE-APACHE",
    }
    if (
        containers["tool"].get("members") != tool
        or containers["tool"].get("path")
        != f"archives/{tool_root}{'.zip' if suffix else '.tar.gz'}"
    ):
        raise RuntimeError(
            "tool archive must contain only the executable, README and license"
        )
    expected_examples = (
        {
            artifact_paths()[name]
            for name in ["schematic-cli", "schematic-sidecar", "schematic-tools"]
        }
        | set(build.get("schemas", {}))
        | {
            "examples/schematic/README.md",
            "README.md",
            "LICENSE-APACHE",
            "examples/build.json",
        }
    )
    example_members = {
        name: ("reports/build.json" if name == "examples/build.json" else name)
        for name in expected_examples
    }
    if (
        containers["examples"].get("members") != example_members
        or containers["examples"].get("path")
        != f"archives/harmos-examples-{target}.zip"
    ):
        raise RuntimeError(
            "example archive is incomplete or contains unrelated payloads"
        )
    snapshot = root / "examples/build.json"
    for container in containers.values():
        archive = root / container["path"]
        if (
            not archive.is_file()
            or archive.is_symlink()
            or _digest(archive) != container.get("sha256")
        ):
            raise RuntimeError("archive is missing or corrupt")
        members = container["members"]
        if set(container.get("digests", {})) != set(members):
            raise RuntimeError("archive member hashes are incomplete")
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as files:
                _check_archive(files)
                if set(files.namelist()) != set(members.values()):
                    raise RuntimeError("archive contains unexpected members")
                for destination, member in members.items():
                    with files.open(member) as content:
                        _consume_member(
                            root,
                            destination,
                            content,
                            container["digests"][destination],
                            extract,
                            files.getinfo(member).external_attr >> 16,
                        )
        else:
            with tarfile.open(archive, "r:gz") as files:
                entries = files.getmembers()
                if (
                    any(not entry.isfile() for entry in entries)
                    or {entry.name for entry in entries} != set(members.values())
                    or len(entries) != len(members)
                ):
                    raise RuntimeError(
                        "tool archive contains unexpected or nonregular members"
                    )
                for destination, member in members.items():
                    content = files.extractfile(member)
                    assert content is not None
                    with content:
                        _consume_member(
                            root,
                            destination,
                            content,
                            container["digests"][destination],
                            extract,
                            files.getmember(member).mode,
                        )

    if snapshot.is_file():
        provenance = json.loads(snapshot.read_text(encoding="utf-8"))
        if provenance != {
            key: value for key, value in build.items() if key != "containers"
        }:
            raise RuntimeError("example provenance differs from its original build")


def _consume_member(
    root: Path, name: str, content, expected: str, extract: bool, mode: int
) -> None:
    # Destination Names Are Checked Against The Fixed Product Inventory Before Reading.
    digest = hashlib.sha256()
    destination = root / name
    if extract:
        if destination.is_symlink() or any(
            parent.is_symlink()
            for parent in destination.parents
            if parent.is_relative_to(root)
        ):
            raise RuntimeError("archive extraction refuses symlink destinations")
        destination.parent.mkdir(parents=True, exist_ok=True)
        with destination.open("wb") as output:
            for chunk in iter(lambda: content.read(262144), b""):
                digest.update(chunk)
                output.write(chunk)
        destination.chmod(mode & 0o755 or 0o644)
    else:
        for chunk in iter(lambda: content.read(262144), b""):
            digest.update(chunk)
    if digest.hexdigest() != expected:
        raise RuntimeError("archive member differs from the staged bytes")


def _tool_versions(repository: Path) -> dict[str, str]:
    versions = {
        name: _output(repository, command) for name, command in VERSION_COMMANDS.items()
    }
    versions["mise"] = _output(repository, ["mise", "--version"])
    return versions


def _build_index(repository: Path, target: Path) -> dict:
    index = json.loads(
        (target / "harmos-build/schemas.json").read_text(encoding="utf-8")
    )
    if (
        index.get("profile") != build_profile()
        or index.get("revision") != _output(repository, ["git", "rev-parse", "HEAD"])
        or index.get("source") != source(repository)
        or index.get("inputs") != release.inputs(repository)
        or index.get("versions") != _tool_versions(repository)
    ):
        raise RuntimeError("build outputs differ from the selected invocation's inputs")
    if set(index.get("outputs", {})) != SCHEMA_PRODUCERS or set(
        index.get("artifacts", {})
    ) != set(BINARIES) | {"schematic_guest"}:
        raise RuntimeError("selected build inventory is incomplete")
    if (
        set(index.get("digests", {})) != set(index["artifacts"])
        or set(index.get("schema_digests", {})) != SCHEMA_PRODUCERS
    ):
        raise RuntimeError("selected build hashes are incomplete")
    for name, filename in index["artifacts"].items():
        selected = Path(filename)
        if (
            not selected.is_absolute()
            or not selected.is_file()
            or selected.is_symlink()
        ):
            raise RuntimeError(
                "selected Cargo artifact must be an absolute regular file"
            )
        if _digest(selected) != index["digests"][name]:
            raise RuntimeError("selected Cargo artifact changed after building")
    for producer, directory in index["outputs"].items():
        schema = Path(directory) / "grpc/operations"
        current = {name: _digest(schema / name) for name in _schema_files(schema)}
        if current != index["schema_digests"][producer]:
            raise RuntimeError("selected Cargo schema changed after building")
    return index


def _schema_files(directory: Path) -> list[str]:
    manifest = directory / "manifest.txt"
    if manifest.is_symlink():
        raise RuntimeError("generated schema manifest must be a regular file")
    names = manifest.read_text(encoding="utf-8").splitlines()
    if names[:2] != ["descriptors.bin", "contracts.json"] or len(set(names)) != len(
        names
    ):
        raise RuntimeError("generated schema manifest is incomplete or duplicated")
    for name in names:
        path = PurePosixPath(name)
        if (
            not name
            or path.is_absolute()
            or ".." in path.parts
            or "\\" in name
            or ":" in name
            or (
                name not in {"descriptors.bin", "contracts.json"}
                and not name.endswith(".proto")
            )
        ):
            raise RuntimeError("unsafe generated schema path")
        selected = directory / name
        if (
            not selected.is_file()
            or selected.is_symlink()
            or any(
                parent.is_symlink()
                for parent in selected.parents
                if parent.is_relative_to(directory)
            )
        ):
            raise RuntimeError("generated schema must contain regular files")
    return names + ["manifest.txt"]


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
    revision = _output(repository, ["git", "rev-parse", "HEAD"])
    runs = _pages(
        f"actions/workflows/full.yml/runs?head_sha={revision}&status=success",
        "workflow_runs",
    )
    if len(runs) != 1:
        raise RuntimeError(
            "no unique successful same-SHA validation run; supply HARMOS_VALIDATION_RUN_ID explicitly"
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
