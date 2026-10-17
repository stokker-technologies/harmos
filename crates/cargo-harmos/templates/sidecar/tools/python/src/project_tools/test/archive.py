# Rendered by stkr project update from templates/python/test/archive.py; sha256:842467871a7e31a4685426bae5ea26ccd8be26c28b9762e348318a25efd799c0
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Compile Once And Bind Cargo Outputs To Their Original Source And Build Inputs.

    python -m <package>.test.archive [--profile dev|release|bench] [-- CARGO_ARGUMENTS...]

Writes `target/nextest/<profile>/archive.tar.zst`, compiler JSON, resolved Cargo metadata and
`archive.json`. The receipt preserves the original revision/provider identity and binds the
exact arguments, effective target, platform, tool executables, configuration, environment,
resolved dependency sources, declared/build-script inputs and output digests. Test lanes
consume the archive without compiling; `--verify -- CARGO_ARGUMENTS...` checks that selection.

`compile(directory, profile, extra)` captures an unavoidable separate workspace/target Cargo
build with the same receipt contract, without a nextest archive. It honors Cargo's target
directory and returns the project-relative receipt path. Source symlinks, ad hoc Cargo config
or manifest overrides and unbounded build-script inputs are refused. Receipts retain original
absolute external-input identities; unavailable or remapped inputs do not establish reuse.

The tree hash is `git write-tree` over a copy of the index with every working-tree change
added (`git add -A`, then `git rm -r --cached --ignore-unmatch .stkr`), so it names exactly
the tracked and unignored content on disk outside the Hub state directory and matches the hash
`stkr` computes.
"""

# === Imports === #

# Standard Library Imports
import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import tomllib
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports

# Local Imports
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ARCHIVE = "archive.tar.zst"
RECEIPT = "archive.json"
MESSAGES = "compiler-artifacts.jsonl"
METADATA = "cargo-metadata.json"
FIXTURES = "target/fixtures/stamp.json"
CHUNK = 1 << 20

# === Types === #

# === Classes === #

# === Public Functions === #


def tree(directory: Path) -> str:
    """The hash of the working tree as `git add -A` would stage it, without touching the index."""
    index = Path(_git(directory, "rev-parse", "--git-path", "index"))
    index = index if index.is_absolute() else directory / index

    with tempfile.TemporaryDirectory(prefix="stkr-tree-") as scratch:
        copy = Path(scratch) / "index"
        if index.is_file():
            shutil.copyfile(index, copy)
        environment = dict(os.environ, GIT_INDEX_FILE=str(copy))
        _git(directory, "add", "-A", environment=environment)
        _git(
            directory,
            "rm",
            "-r",
            "-q",
            "--cached",
            "--ignore-unmatch",
            ".stkr",
            environment=environment,
        )
        return _git(directory, "write-tree", environment=environment)


def revision(directory: Path) -> str | None:
    """The checked-out commit, or None outside a repository with history."""
    try:
        return _git(directory, "rev-parse", "--verify", "HEAD")
    except policy.TestError:
        return None


def published(directory: Path, current: str) -> str | None:
    """The commit evidence may cite: HEAD when the tree hash is exactly its tree."""
    head = revision(directory)
    if head is None or _git(directory, "rev-parse", "HEAD^{tree}") != current:
        return None
    return head


def location(directory: Path, profile: str) -> Path:
    return directory / "target" / "nextest" / profile / ARCHIVE


def build(directory: Path, profile: str, extra: Sequence[str] = ()) -> dict:
    """Compile and archive every workspace target with its original producer receipt."""
    return _produce(directory, profile, extra, archived=True)


def compile(directory: Path, profile: str, extra: Sequence[str] = ()) -> dict:
    """Capture one separate Cargo workspace/target invocation using the same producer contract."""
    return _produce(directory, profile, extra, archived=False)


def listed(directory: Path, profile: str) -> list[str]:
    """The test binary names of the workspace, from nextest's listing of the already built targets."""
    command = ["cargo", "nextest", "list", "--archive-file", str(location(directory, profile)),
               "--workspace-remap", str(directory), "--extract-to", str(location(directory, profile).parent / "listing"),
               "--extract-overwrite", "--list-type", "binaries-only", "--message-format", "json"]
    try:
        result = subprocess.run(command, cwd=directory, capture_output=True, text=True, check=False)
    except OSError as error:
        raise policy.TestError(f"cargo nextest is unavailable: {error}") from error
    if result.returncode != 0:
        raise policy.TestError(f"nextest could not list the test binaries: {result.stderr.strip()}")
    binaries = json.loads(result.stdout).get("rust-binaries", {}).values()
    return sorted({binary["binary-name"] for binary in binaries if binary.get("kind") == "test"})


def binaries(directory: Path, profile: str) -> list[str]:
    """The test binary names recorded with the archive of this profile."""
    receipt = location(directory, profile).with_name(RECEIPT)
    if not receipt.is_file():
        return []
    return list(json.loads(receipt.read_text(encoding="utf-8")).get("binaries", []))


def require(directory: Path, profile: str, current: str, arguments: Sequence[str] = ()) -> Path:
    """The archive of this profile, refused when absent or built from another tree."""
    path = location(directory, profile)
    receipt = path.with_name(RECEIPT)
    if not path.is_file() or not receipt.is_file():
        raise policy.TestError(f"no {profile} nextest archive; run the build that produces {path.relative_to(directory)}")
    record = json.loads(receipt.read_text(encoding="utf-8"))
    if record.get("tree") != current or record.get("profile") != profile:
        raise policy.TestError(f"the {profile} nextest archive was built from another tree; rebuild it")
    if record.get("schema") != 1 or "dependencies" not in record:
        raise policy.TestError("the nextest archive has no complete input receipt; rebuild it")
    if record.get("arguments") != list(arguments):
        raise policy.TestError("the nextest archive uses another Cargo argument selection")
    if record.get("revision") != revision(directory):
        raise policy.TestError("the nextest archive belongs to another source revision")
    if record.get("inputs") != _inputs(directory):
        raise policy.TestError("the nextest archive uses another platform, environment or configuration")
    project = Path(_git(directory, "rev-parse", "--show-toplevel"))
    resolution = record.get("resolution", {})
    lock = project / resolution.get("lock", "Cargo.lock")
    if not resolution.get("packages") or resolution.get("sha256") != _digest(lock):
        raise policy.TestError("the nextest archive uses another locked dependency resolution")
    if "watched" not in record or record.get("watched_inputs") != _watched_inputs(record["watched"]):
        raise policy.TestError("the nextest archive build-script inputs changed")
    for name, digest in record.get("dependencies", {}).items():
        if _source(Path(name)) != digest:
            raise policy.TestError(f"the nextest archive dependency changed: {name}")
    for key, name in (("metadata", METADATA), ("messages", MESSAGES)):
        artifact = path.with_name(name)
        if record.get(key) != {"path": _relative(project, artifact), "sha256": _digest(artifact)}:
            raise policy.TestError(f"the nextest archive {key} does not match its build receipt")
    if record.get("sha256") != _digest(path):
        raise policy.TestError(f"the {profile} nextest archive digest does not match its build receipt")
    if record.get("toolchain") != _toolchain(directory):
        raise policy.TestError(f"the {profile} nextest archive uses another Rust toolchain")
    return path


def verify_outputs(record: dict, paths: Sequence[Path] | None = None) -> None:
    """Require unchanged producer output bytes before staging selected regular binaries or generated files.

    Use the record returned by build/compile, or first require the persisted archive's inputs.
    Omitting paths checks every expected output, including entries deleted after production.
    """
    if record.get("schema") != 1 or not record.get("artifacts") or "generated" not in record:
        raise policy.TestError("no complete producer output map; rebuild the artifacts")
    expected = {**record["artifacts"], **record["generated"]}
    selected = list(expected) if paths is None else [str(path) for path in paths]
    for name in selected:
        if name not in expected:
            raise policy.TestError(f"output was not recorded by the compiler producer: {name}")
        if _source(Path(name)) != expected[name]:
            raise policy.TestError(f"output changed after compiler production: {name}")


def fixtures(directory: Path, current: str) -> None:
    """Refuse a fixture stamp from another tree; projects without fixtures have no stamp."""
    stamp = directory / FIXTURES
    if not stamp.is_file():
        return
    record = json.loads(stamp.read_text(encoding="utf-8"))
    if record.get("tree") != current:
        raise policy.TestError(f"{FIXTURES} is stale; rebuild the fixtures")


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build The Workspace Nextest Archive")
    parser.add_argument("--profile", choices=(*policy.PROFILES, "bench"), default="dev")
    parser.add_argument("--verify", action="store_true", help="Require The Existing Archive Without Compiling")
    parser.add_argument(
        "extra",
        nargs=argparse.REMAINDER,
        help="Cargo arguments after --, such as --all-features",
    )
    options = parser.parse_args(arguments)
    extra = options.extra[1:] if options.extra[:1] == ["--"] else options.extra
    try:
        directory = policy.root()
        if options.verify:
            path = require(directory, options.profile, tree(directory), extra)
            record = json.loads(path.with_name(RECEIPT).read_text(encoding="utf-8"))
        else:
            record = build(directory, options.profile, extra)
    except (policy.TestError, OSError, ValueError) as error:
        print(f"archive: {error}", file=sys.stderr)
        return 1
    print(json.dumps(record, indent=2))
    return 0


# === Private Functions === #


def _produce(directory: Path, profile: str, extra: Sequence[str], *, archived: bool) -> dict:
    if profile not in (*policy.PROFILES, "bench"):
        raise policy.TestError("build profile must be dev, release or bench")
    if any(argument in ("--config", "--manifest-path") or argument.startswith(("--config=", "--manifest-path=")) for argument in extra):
        raise policy.TestError("build inputs require repository-owned Cargo configuration and the current workspace manifest")
    directory = directory.resolve()
    project = Path(_git(directory, "rev-parse", "--show-toplevel"))
    before = tree(project)
    inputs = _inputs(directory)
    target = _target(directory, extra)
    metadata_command = ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target, *_features(extra)]
    metadata = _output(directory, metadata_command)
    resolved = json.loads(metadata)
    if archived:
        path = location(directory, profile)
    else:
        workspace = directory.relative_to(project)
        target_name = hashlib.sha256(target.encode()).hexdigest()[:16]
        path = Path(resolved["target_directory"]) / "cargo" / workspace / target_name / profile / "build.json"
    receipt = path.with_name(RECEIPT) if archived else path
    path.parent.mkdir(parents=True, exist_ok=True)
    receipt.unlink(missing_ok=True)
    messages = path.with_name(MESSAGES)
    path.with_name(METADATA).write_text(metadata, encoding="utf-8")
    external = sorted({str(Path(package["manifest_path"]).parent) for package in resolved["packages"]
                       if not Path(package["manifest_path"]).is_relative_to(project)})
    observed = {name: _source(Path(name)) for name in external}
    dependencies = {str(Path(package["manifest_path"]).parent): observed[str(Path(package["manifest_path"]).parent)]
                    for package in resolved["packages"]
                    if package["source"] is None and str(Path(package["manifest_path"]).parent) in observed}
    immutable = [Path(package["manifest_path"]).parent for package in resolved["packages"] if package["source"] is not None]
    lock = Path(resolved["workspace_root"]) / "Cargo.lock"
    resolution = {"lock": _relative(project, lock), "sha256": _digest(lock),
                  "packages": sorted((package["id"], package["source"]) for package in resolved["packages"])}
    policy_root, configuration = _configuration(directory)
    declared = configuration.get("hooks", {}).get("validation_inputs", [])
    bounded = [*(Path(name) for name in external), *(policy_root / name for name in declared)]
    sources = {project / name for name in _git(project, "ls-files", "--cached", "--others", "--exclude-standard", "-z").split("\0") if name}
    if linked := next((file for file in sources if file.is_symlink()), None):
        raise policy.TestError(f"unbounded symbolic link in workspace source: {linked}")
    environment = {name: hashlib.sha256(value.encode()).hexdigest() for name, value in os.environ.items()}
    original = revision(project)
    command = [
        "cargo", "nextest", "archive", "--locked", "--workspace", "--all-targets",
        "--cargo-profile", profile, "--cargo-message-format", "json", "--archive-file", str(path), *extra,
    ]
    if not archived:
        command = ["cargo", "build", "--locked", "--profile", profile, "--message-format=json", *extra]
    with messages.open("w", encoding="utf-8") as output:
        status = _run(command, directory, output)
    if status != 0:
        raise policy.TestError("nextest archive build failed")
    names = listed(directory, profile) if archived else []
    if json.loads(_output(directory, metadata_command)) != resolved:
        raise policy.TestError("the resolved dependency graph changed during compilation")
    watched = _watched(directory, messages, resolved)
    for name in watched["files"]:
        if not _covered(Path(name), bounded, sources):
            raise policy.TestError(f"build-script input was not bounded before compilation: {name}; declare hooks.validation_inputs and rebuild")
    # Immutable Registry/Git Inputs Are Identified By The Locked Resolution, Not A Consumer's Optional Cargo Cache.
    immutable_watches = {name: _source(Path(name)) for name in watched["files"]
                         if any(Path(name).resolve().is_relative_to(root.resolve()) for root in immutable)}
    watched["files"] = [name for name in watched["files"] if name not in immutable_watches]
    watched_inputs = _watched_inputs(watched)
    if any(watched_inputs["environment"][name] != environment.get(name) for name in watched["environment"]):
        raise policy.TestError("a build-script environment input changed during compilation")
    if any(_source(Path(name)) != digest for name, digest in observed.items()):
        raise policy.TestError("a resolved dependency changed during compilation")
    if _digest(lock) != resolution["sha256"]:
        raise policy.TestError("the Cargo lock changed during compilation")
    if _inputs(directory) != inputs or revision(project) != original:
        raise policy.TestError("the build environment or source revision changed while the archive was built")

    # Edits During Compilation Make The Archive Unattributable.
    if tree(project) != before:
        raise policy.TestError("the source tree changed while the archive was built")
    record = {
        "schema": 1,
        "kind": "nextest" if archived else "cargo",
        "workspace": directory.relative_to(project).as_posix(),
        "receipt": _relative(project, receipt),
        "tree": before,
        "revision": original,
        "published_revision": published(project, before),
        "provider": {key: os.environ[key] for key in ("GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_JOB", "CI_JOB_ID", "CI_PIPELINE_ID") if key in os.environ},
        "inputs": inputs,
        "dependencies": dependencies,
        "dependency_inputs": observed,
        "resolution": resolution,
        "immutable_watches": immutable_watches,
        "watched": watched,
        "watched_inputs": watched_inputs,
        "metadata": {"path": _relative(project, path.with_name(METADATA)), "sha256": _digest(path.with_name(METADATA))},
        "messages": {"path": _relative(project, messages), "sha256": _digest(messages)},
        "target": target,
        "profile": profile,
        "arguments": list(extra),
        "command": command,
        "binaries": names,
        "toolchain": _toolchain(directory),
        "artifacts": _artifacts(messages),
        "generated": _generated(messages),
        "sha256": _digest(path) if archived else None,
    }
    receipt.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return record


def _git(directory: Path, *arguments: str, environment: dict[str, str] | None = None) -> str:
    try:
        result = subprocess.run(
            ["git", *arguments],
            cwd=directory,
            env=environment,
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=False,
        )
    except OSError as error:
        raise policy.TestError(f"git is unavailable: {error}") from error
    if result.returncode != 0:
        raise policy.TestError(f"git {arguments[0]} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def _run(command: Sequence[str], directory: Path, output=None) -> int:
    try:
        return subprocess.run(list(command), cwd=directory, stdout=output, check=False).returncode
    except OSError as error:
        raise policy.TestError(f"{command[0]} is unavailable: {error}") from error


def _output(directory: Path, command: Sequence[str]) -> str:
    result = subprocess.run(command, cwd=directory, capture_output=True, text=True, encoding="utf-8", check=False)
    if result.returncode != 0:
        raise policy.TestError(f"{command[0]} failed: {result.stderr.strip()}")
    return result.stdout


def _toolchain(directory: Path) -> str:
    return _output(directory, ["rustc", "-Vv"]).strip()


def _inputs(directory: Path) -> dict:
    # Only Digests Leave The Process: Configuration And Environment May Contain Secrets.
    environment = {key: hashlib.sha256(value.encode()).hexdigest() for key, value in os.environ.items()
                   if key.startswith(("CARGO_", "RUST", "PYTHON", "UV_", "npm_config_", "PKG_CONFIG_"))
                   or key in ("PATH", "PATHEXT", "CC", "CXX", "AR", "LD", "CFLAGS", "CXXFLAGS", "LDFLAGS",
                              "MISE_ENV", "MISE_PROFILE", "MISE_CONFIG_FILE", "MISE_DATA_DIR",
                              "INCLUDE", "LIB", "LIBPATH", "CPATH", "LIBRARY_PATH", "SDKROOT", "MACOSX_DEPLOYMENT_TARGET")}
    files = {}
    for ancestor in (directory, *directory.parents):
        for name in (".cargo/config", ".cargo/config.toml", "rust-toolchain", "rust-toolchain.toml"):
            file = ancestor / name
            files[str(file)] = _digest(file) if file.is_file() else None
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    for name in ("config", "config.toml"):
        file = cargo_home / name
        files[str(file)] = _digest(file) if file.is_file() else None
    policy_root, configuration = _configuration(directory)
    for name in configuration.get("hooks", {}).get("validation_inputs", []):
        file = policy_root / name
        files[name] = _source(file)
    return {"platform": [sys.platform, platform.machine(), platform.platform()], "environment": environment, "configuration": files,
            "rustc": _toolchain(directory), "cargo": _output(directory, ["cargo", "-V"]).strip(), "tools": _tools(directory)}


def _configuration(directory: Path) -> tuple[Path, dict]:
    for ancestor in (directory, *directory.parents):
        path = ancestor / "stkr.toml"
        if path.is_file():
            return ancestor, tomllib.loads(path.read_text(encoding="utf-8"))
    return directory, {}


def _relative(root: Path, path: Path) -> str:
    return path.relative_to(root).as_posix() if path.is_relative_to(root) else str(path)


def _artifacts(messages: Path) -> dict:
    files = set()
    for line in messages.read_text(encoding="utf-8").splitlines():
        message = json.loads(line)
        if message.get("reason") == "compiler-artifact":
            files.update(message["filenames"])
            if executable := message.get("executable"):
                files.add(executable)
    return {name: _source(Path(name)) for name in sorted(files)}


def _generated(messages: Path) -> dict:
    files = set()
    for line in messages.read_text(encoding="utf-8").splitlines():
        message = json.loads(line)
        if message.get("reason") != "build-script-executed":
            continue
        directory = Path(message["out_dir"])
        for file in directory.rglob("*"):
            if file.is_symlink():
                raise policy.TestError(f"unbounded symbolic link in generated build output: {file}")
            if file.is_file():
                files.add(str(file))
    return {name: _digest(Path(name)) for name in sorted(files)}


def _tools(directory: Path) -> dict:
    tools = {}
    selected = {name: name for name in ("rustc", "cargo", "cc", "c++", "ld", "ar", "cl", "link", "pkg-config")}
    selected.update({name: value for name, value in os.environ.items()
                     if name in ("RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CC", "CXX", "AR", "LD")
                     or name.startswith("CARGO_TARGET_") and name.endswith("_LINKER")})
    for config in _cargo_configs(directory):
        document = tomllib.loads(config.read_text(encoding="utf-8"))
        if "include" in document:
            raise policy.TestError("Cargo configuration includes require an explicitly bounded configuration")
        for name in ("rustc", "rustc-wrapper", "rustc-workspace-wrapper"):
            if value := document.get("build", {}).get(name):
                selected[f"{config}:{name}"] = value
        for target, settings in document.get("target", {}).items():
            if value := settings.get("linker"):
                selected[f"{config}:{target}:linker"] = value
    for name, value in selected.items():
        executable = shutil.which(value)
        if executable is None and name not in ("cc", "c++", "ld", "ar", "cl", "link", "pkg-config"):
            raise policy.TestError(f"cannot identify effective {name} executable; declare an executable path")
        tools[name] = _digest(Path(executable).resolve()) if executable else None
    return tools


def _cargo_configs(directory: Path) -> list[Path]:
    configs = []
    for ancestor in (directory, *directory.parents):
        # Native Cargo Prefers The Extensionless File When Both Exist.
        for name in (".cargo/config", ".cargo/config.toml"):
            config = ancestor / name
            if config.is_file():
                configs.append(config)
                break
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    for name in ("config", "config.toml"):
        if (config := cargo_home / name).is_file():
            configs.append(config)
            break
    return list(dict.fromkeys(configs))


def _covered(path: Path, bounded: Sequence[Path], sources: set[Path]) -> bool:
    path = path.resolve()
    if any(path == root.resolve() or path.is_relative_to(root.resolve()) for root in bounded):
        return True
    if path.is_file():
        return path in sources
    if path.is_dir():
        return all(file in sources for file in path.rglob("*") if file.is_file() or file.is_symlink())
    return False


def _source(path: Path) -> str:
    if path.is_symlink():
        raise policy.TestError(f"unbounded symlink in declared build input: {path}")
    if path.is_file():
        return _digest(path)
    if not path.is_dir():
        raise policy.TestError(f"missing declared build input: {path}")
    # External Path Dependencies Must Be Inspectable At Consumption; Never Assume Saved Metadata Proves Equivalence.
    entries = []
    for file in sorted(path.rglob("*")):
        relative = file.relative_to(path)
        if file.is_symlink():
            raise policy.TestError(f"unbounded symlink in declared build input: {file}")
        if file.is_file():
            entries.append((relative.as_posix(), _digest(file)))
    return hashlib.sha256(json.dumps(entries).encode()).hexdigest()


def _watched(directory: Path, messages: Path, metadata: dict) -> dict:
    packages = {package["id"]: Path(package["manifest_path"]).parent for package in metadata["packages"]}
    files, environment = set(), set()
    for line in messages.read_text(encoding="utf-8").splitlines():
        message = json.loads(line)
        if message.get("reason") != "build-script-executed":
            continue
        output = Path(message["out_dir"]).parent / "output"
        if not output.is_file():
            raise policy.TestError("build-script input declarations are unavailable; rebuild with inspectable Cargo outputs")
        for declaration in output.read_text(encoding="utf-8").splitlines():
            declaration = declaration.removeprefix("cargo::").removeprefix("cargo:")
            if declaration.startswith("rerun-if-env-changed="):
                environment.add(declaration.split("=", 1)[1])
            elif declaration.startswith("rerun-if-changed="):
                path = Path(declaration.split("=", 1)[1])
                if not path.is_absolute():
                    path = packages[message["package_id"]] / path
                files.add(str(path))
    return {"files": sorted(files), "environment": sorted(environment)}


def _watched_inputs(watched: dict) -> dict:
    return {"files": {name: _source(Path(name)) for name in watched["files"]},
            "environment": {name: hashlib.sha256(os.environ[name].encode()).hexdigest() if name in os.environ else None
                            for name in watched["environment"]}}


def _features(arguments: Sequence[str]) -> list[str]:
    selected = []
    values = iter(arguments)
    for argument in values:
        if argument in ("--all-features", "--no-default-features") or argument.startswith("--features="):
            selected.append(argument)
        elif argument in ("--features", "-F"):
            try:
                selected.extend(["--features", next(values)])
            except StopIteration as error:
                raise policy.TestError("Cargo feature selection is missing its value") from error
        elif argument.startswith("-F"):
            selected.extend(["--features", argument[2:]])
    return selected


def _target(directory: Path, arguments: Sequence[str]) -> str:
    targets = []
    values = iter(arguments)
    for argument in values:
        if argument == "--target":
            try:
                targets.append(next(values))
            except StopIteration as error:
                raise policy.TestError("Cargo target selection is missing its value") from error
        elif argument.startswith("--target="):
            targets.append(argument.split("=", 1)[1])
    if len(targets) > 1:
        raise policy.TestError("multiple Cargo targets require separate producer invocations")
    if targets:
        return targets[0]
    if os.environ.get("CARGO_BUILD_TARGET"):
        return os.environ["CARGO_BUILD_TARGET"]
    for config in _cargo_configs(directory):
        value = tomllib.loads(config.read_text(encoding="utf-8")).get("build", {}).get("target")
        if value is not None:
            if not isinstance(value, str):
                raise policy.TestError("multiple Cargo build targets require separate archive producers")
            return value
    for line in _toolchain(directory).splitlines():
        if line.startswith("host: "):
            return line.removeprefix("host: ")
    raise policy.TestError("Rust toolchain did not identify its native target")


def _digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(CHUNK), b""):
            digest.update(chunk)
    return digest.hexdigest()


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
