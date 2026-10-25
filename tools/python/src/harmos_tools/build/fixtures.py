# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Capture And Stage Exact Cargo And Offline Fixture Inputs"""

# === Imports === #

# Standard Library Imports
import hashlib
import importlib.resources
import json
import os
import shutil
import stat
import subprocess
import sys
from pathlib import Path

# Third Party Imports

# Local Imports
from .. import temporary
from . import cargo

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[5]
TARGET = ROOT / "target"
GENERATED = TARGET / "generated"
SCHEMATIC = ROOT / "examples/schematic/Cargo.toml"
GUESTS = {"schematic-guest": SCHEMATIC, "harmos-sandbox-guest": ROOT / "crates/harmos-guest/tests/fixtures/sandbox/Cargo.toml"}
LIBRARIES = ("harmos", "harmos_sidecar", "harmos_macros", "serde")

# === Types === #

# === Classes === #

# === Public Functions === #


# === Private Functions === #


def capture(name: str, directory: Path, profile: str, extra: list[str], scratch: Path) -> dict:
    """Compile One Cargo Selection And Record The Collection Input `cargo harmos` Requires."""
    destination = scratch / "captures" / name / profile
    shutil.rmtree(destination, ignore_errors=True)
    destination.mkdir(parents=True)

    rustc = subprocess.check_output(
        ["rustc", "-Vv"],
        cwd=directory,
        text=True,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        stdin=sys.stdin,
        stderr=sys.stderr,
    )
    hosts = (line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    host = next(hosts)
    target = extra[extra.index("--target") + 1] if "--target" in extra else host
    platforms = [option for triple in dict.fromkeys((target, host)) for option in ("--filter-platform", triple)]
    features = ["--all-features"] if "--all-features" in extra else []

    metadata = destination / "metadata.json"
    with metadata.open("w", encoding="utf-8") as output:
        subprocess.run(
            ["cargo", "metadata", "--locked", "--format-version", "1", *platforms, *features],
            cwd=directory,
            stdout=output,
            check=True,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
            stdin=sys.stdin,
            stderr=sys.stderr,
        )
    messages = destination / "messages.json"
    arguments = ["--locked", "--profile", profile, *extra]
    cargo.clippy(ROOT, ["--manifest-path", str(directory / "Cargo.toml"), *arguments])
    command = ["cargo", "build", *arguments, "--message-format=json"]
    with messages.open("w", encoding="utf-8") as output:
        completed = subprocess.run(
            command,
            cwd=directory,
            stdout=output,
            check=False,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
            stdin=sys.stdin,
            stderr=sys.stderr,
        )
    if completed.returncode:
        raise RuntimeError(f"cargo build failed for {name}")

    content = messages.read_text(encoding="utf-8")
    lines = [json.loads(line) for line in content.splitlines() if line.startswith("{")]
    artifacts = [message for message in lines if message.get("reason") == "compiler-artifact"]
    produced = {path for artifact in artifacts for path in artifact["filenames"]}
    executables = {artifact["executable"] for artifact in artifacts if artifact.get("executable")}
    produced.update(executables)
    generated = {
        str(path)
        for message in lines
        if message.get("reason") == "build-script-executed"
        for path in Path(message["out_dir"]).rglob("*")
        if path.is_file()
    }

    record = {
        "schema": 1,
        "profile": profile,
        "target": target,
        "command": command,
        "inputs": {"rustc": rustc},
        "messages": {"path": str(messages), "sha256": digest(messages)},
        "metadata": {"path": str(metadata), "sha256": digest(metadata)},
        "artifacts": _digests(produced),
        "generated": _digests(generated),
    }
    receipt = destination / "capture.json"
    receipt.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    record["receipt"] = str(receipt)
    return record


def messages(record: dict) -> list[dict]:
    content = Path(record["messages"]["path"]).read_text(encoding="utf-8")
    return [json.loads(line) for line in content.splitlines() if line.startswith("{")]


def collect(tool: Path, manifest: Path, package: str, backend: str, profile: str, capture: dict, scratch: Path) -> tuple[dict, Path]:
    destination = scratch / "collected" / package
    shutil.rmtree(destination, ignore_errors=True)
    destination.mkdir(parents=True)
    result = destination / "build.json"
    command = [
        str(tool),
        "harmos",
        backend,
        "build",
        "--manifest-path",
        str(manifest),
        "-p",
        package,
        "--profile",
        profile,
        "--target",
        capture["target"],
        "--from-cargo-json",
        capture["messages"]["path"],
        "--metadata-json",
        capture["metadata"]["path"],
        "--archive-json",
        capture["receipt"],
        "--json",
    ]
    with result.open("w", encoding="utf-8") as output:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            stdout=output,
            check=False,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
            stdin=sys.stdin,
            stderr=sys.stderr,
        )
    if completed.returncode:
        raise RuntimeError(f"cargo harmos could not collect {package}")
    return json.loads(result.read_text(encoding="utf-8")), result


def offline_inputs(destination: Path) -> Path:
    """Export The Locked Native Dependencies The Offline Authoring Stories Build From."""
    manifests = sorted({ROOT / "Cargo.toml", SCHEMATIC, *GUESTS.values()})
    vendor = ["cargo", "vendor", "--locked", "--versioned-dirs"]
    synced = (manifest for manifest in manifests if manifest != ROOT / "Cargo.toml")
    for manifest in synced:
        vendor += ["--sync", str(manifest)]
    vendor.append(str(destination / "vendor"))
    destination.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        vendor,
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        check=True,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        stdin=sys.stdin,
        stderr=sys.stderr,
    )
    projects = [ROOT / "tools/python", *(ROOT / f"crates/cargo-harmos/templates/{kind}/tools/python" for kind in ("sidecar", "guest"))]
    requirements = []
    for index, project in enumerate(projects):
        selected = destination / ("requirements.txt" if index == 0 else f"starter-{project.parents[1].name}.txt")
        _requirements(project, selected)
        requirements.append(selected)
    bundled = importlib.resources.files("ensurepip").joinpath("_bundled")
    wheels = [path for path in bundled.iterdir() if path.name.startswith("pip-") and path.name.endswith(".whl")]
    if len(wheels) != 1:
        raise RuntimeError("the pinned Python must provide its bundled pip wheel to stage offline inputs")
    launch = "import runpy,sys; sys.path.insert(0,sys.argv.pop(1)); runpy.run_module('pip',run_name='__main__')"
    # Each Project Locks Its Own Closure, So Each Downloads Separately Into The Shared Wheel Directory.
    download = [sys.executable, "-c", launch, str(wheels[0]), "download", "--dest", str(destination / "wheels"), "--require-hashes"]
    for selected in requirements:
        subprocess.run(
            [*download, "--requirement", str(selected)],
            cwd=ROOT,
            check=True,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
            stdin=sys.stdin,
            stdout=sys.stdout,
            stderr=sys.stderr,
        )
    # Hatchling Requests `editables` Only For Editable Installs, So No Project Lock Names It.
    subprocess.run(
        [*download, "--no-deps", "--requirement", str(ROOT / "tools/python/editables.txt")],
        cwd=ROOT,
        check=True,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
    )
    return destination


def _requirements(project: Path, destination: Path) -> None:
    """Project Native Or Suffixed Starter Workspace Metadata Without Publishing A Second Source Tree."""
    suffix = "" if (project / "pyproject.toml").is_file() else ".tmpl"
    workspace = project.parents[1]
    environment = dict(os.environ)
    environment.pop("UV_PROJECT_ENVIRONMENT", None)
    with temporary.directory() as projected, destination.open("w", encoding="utf-8") as output:
        member = projected / "tools/python"
        member.mkdir(parents=True)
        shutil.copy2(workspace / f"pyproject.toml{suffix}", projected / "pyproject.toml")
        shutil.copy2(workspace / f"uv.lock{suffix}", projected / "uv.lock")
        shutil.copy2(project / f"pyproject.toml{suffix}", member / "pyproject.toml")
        export = ["uv", "export", "--locked", "--all-groups", "--no-emit-project", "--project", str(member)]
        subprocess.run(
            export,
            cwd=ROOT,
            stdout=output,
            check=True,
            env=environment,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
            stdin=sys.stdin,
            stderr=sys.stderr,
        )


def generate_python(tool: Path, build: Path, protoc: Path, files: dict) -> None:
    """Generate The Schematic Sidecar's Python Client, Which `check:python:generated` Checks."""
    metadata = json.loads(build.read_text(encoding="utf-8"))
    catalogs = {schema["catalog"] for schema in metadata["schemas"] if schema["producer"] == "schematic-sidecar"}
    if len(catalogs) != 1:
        raise RuntimeError(f"expected one schematic-sidecar catalog, found {sorted(catalogs)}")

    protoc.chmod(protoc.stat().st_mode | stat.S_IXUSR)
    output = GENERATED / "python"
    shutil.rmtree(output, ignore_errors=True)

    command = [
        str(tool),
        "harmos",
        "bindings",
        "python",
        "--build-json",
        str(build),
        "--producer",
        "schematic-sidecar",
        "--catalog",
        catalogs.pop(),
        "--output",
        str(output),
        "--distribution",
        "schematic-client",
        "--module",
        "schematic_client",
        "--package-version",
        "0.0.0",
        "--description",
        "Schematic Sidecar Client",
        "--license",
        "Apache-2.0",
        "--protoc",
        str(protoc),
    ]
    subprocess.run(
        command,
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        check=True,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        stdin=sys.stdin,
        stderr=sys.stderr,
    )
    generated = sorted(output.rglob("*"))
    generated_files = (path for path in generated if path.is_file())
    for path in generated_files:
        files[path.relative_to(ROOT).as_posix()] = digest(path)


def stage_build(build: dict, collected: Path, package: str, files: dict, fixtures: Path) -> tuple[str, str]:
    selected = [artifact for artifact in build["artifacts"] if artifact["producer"] == package]
    if len(selected) != 1:
        raise RuntimeError(f"{package} did not produce exactly one artifact: {selected}")
    artifact = selected[0]
    root = fixtures / "builds" / package
    source = Path(artifact["path"])
    destination = root / source.relative_to(ROOT)
    if copy(source, destination, files) != artifact["sha256"]:
        raise RuntimeError("typed artifact digest differs from its collected build")
    for schema in build["schemas"]:
        _stage_schema(schema, root, files)
    # Original Build JSON Remains Byte-Exact; Packaging Relocates Its Typed Paths.
    receipt = fixtures / "receipts" / f"{package}.json"
    copy(collected, receipt, files)
    return destination.relative_to(fixtures).as_posix(), receipt.relative_to(fixtures).as_posix()


def _stage_schema(schema: dict, root: Path, files: dict) -> None:
    source = Path(schema["directory"])
    for name, expected in schema["files"].items():
        original = source / name
        copied = copy(original, root / original.relative_to(ROOT), files)
        verify(copied, expected, "typed schema digest differs from its collected build")


def copy(source: Path, destination: Path, files: dict) -> str:
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    checksum = digest(destination)
    files[destination.relative_to(ROOT).as_posix()] = checksum
    return checksum


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def _digests(names: set[str]) -> dict[str, str]:
    result = {}
    for name in sorted(names):
        path = Path(name)
        result[name] = digest(path)
    return result


def verify(actual: str, expected: str, reason: str) -> None:
    if actual != expected:
        raise RuntimeError(reason)


def executable(messages: list[dict], name: str) -> Path:
    found = {
        message["executable"]
        for message in messages
        if message.get("reason") == "compiler-artifact"
        and message["target"]["name"] == name
        and message.get("executable")
        and not message["profile"]["test"]
    }
    if len(found) != 1:
        raise RuntimeError(f"expected one {name} executable, found {sorted(found)}")
    return Path(found.pop())


def libraries(messages: list[dict], files: dict, fixtures: Path) -> dict[str, str]:
    """Transfer exact compiler-emitted libraries needed by the authoring refusal stories.

    Prefer the build whose features include every other's. Equal-feature host and
    shipping builds resolve against the authoritative shipping Harmos profile.
    """
    found: dict[str, dict[str, tuple[set[str], dict]]] = {name: {} for name in LIBRARIES}
    destination = fixtures / "libraries"
    destination.mkdir()

    inputs = (
        (message, filename) for message in messages if message.get("reason") == "compiler-artifact" for filename in message["filenames"]
    )
    staged = (_library(message, filename, files, fixtures) for message, filename in inputs)
    for name, library, features, profile in filter(None, staged):
        found[name][library] = features, profile

    shipping = _widest(found["harmos"])
    profile = found["harmos"][shipping][1] if shipping is not None else None
    selected = {name: _widest(builds, profile) for name, builds in found.items()}
    ambiguous = {name: sorted(found[name]) for name, path in selected.items() if path is None}
    if ambiguous:
        raise RuntimeError(f"compiler libraries are missing or ambiguous: {ambiguous}")
    return {name: path for name, path in selected.items() if path is not None}


def _library(message: dict, filename: str, files: dict, fixtures: Path) -> tuple[str, str, set[str], dict] | None:
    source = Path(filename)
    if source.suffix not in {".rlib", ".rmeta", ".so", ".dylib", ".dll"}:
        return None
    # Matching Emitted Libraries Already Carry Their Metadata.
    companion = source.with_suffix(".rlib")
    emitted = {Path(name) for name in message["filenames"]}
    if source.suffix == ".rmeta" and companion in emitted:
        return None
    destination = fixtures / "libraries" / source.name
    if not destination.exists():
        copy(source, destination, files)

    name = message["target"]["name"]
    if name not in LIBRARIES:
        return None
    suffix = (".dll" if os.name == "nt" else ".dylib" if sys.platform == "darwin" else ".so") if name == "harmos_macros" else ".rlib"
    if source.suffix != suffix:
        return None
    return name, destination.relative_to(fixtures).as_posix(), set(message["features"]), message["profile"]


def _widest(builds: dict[str, tuple[set[str], dict]], profile: dict | None = None) -> str | None:
    features = [enabled for enabled, _ in builds.values()]
    required = set().union(*features)
    widest = [path for path, (features, _) in builds.items() if required <= features]
    if len(widest) > 1 and profile is not None:
        widest = [path for path in widest if builds[path][1] == profile]
    return widest[0] if len(widest) == 1 else None


# === Entry Point === #
