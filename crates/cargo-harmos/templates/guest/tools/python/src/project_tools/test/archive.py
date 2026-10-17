# Rendered by stkr project update from templates/python/test/archive.py; sha256:ee02ce4220cdf136c228ff519f0d867017f5a68759b270f35b7d3632a1bf5e95
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
from collections.abc import Mapping, Sequence
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


def build(directory: Path, profile: str, extra: Sequence[str] = (), *, parents: Sequence[dict] = (), inputs: Sequence[Path] = ()) -> dict:
    """Compile and archive every workspace target with its original producer receipt."""
    return _produce(directory, profile, extra, archived=True, parents=parents, inputs=inputs)


def compile(directory: Path, profile: str, extra: Sequence[str] = (), *, parents: Sequence[dict] = (), inputs: Sequence[Path] = ()) -> dict:
    """Capture one separate Cargo workspace/target invocation using the same producer contract."""
    return _produce(directory, profile, extra, archived=False, parents=parents, inputs=inputs)


def native(
    directory: Path, profile: str, role: str, commands: Sequence[Sequence[str]], outputs: Sequence[Path], *,
    locks: Sequence[Path], inputs: Sequence[Path] = (), installed: Sequence[Path] = (), parents: Sequence[dict] = (),
    stdout: Mapping[int, Path] | None = None,
) -> dict:
    """Capture an actual closed native producer; product tasks retain invocation and ordering ownership.

    Roles are dependency-export (locked cargo vendor/uv export/hashed pip download), frontend
    (Node build), python-package (offline uv build with a prepared backend), and package
    (a parent-produced native tool). rust-package captures locked, noncompiling Cargo packaging.
    Outputs must be fresh owned files/directories. Inputs copied from parents retain their
    original digests; installed dependency snapshots are backed by explicit native lockfiles.
    """
    if role not in ("dependency-export", "frontend", "python-package", "rust-package", "package") or not commands or not outputs or not locks:
        raise policy.TestError("native production requires a closed role, invocations, outputs and locked inputs")
    directory = directory.resolve()
    project = Path(_git(directory, "rev-parse", "--show-toplevel"))
    commands = [[str(argument) for argument in command] for command in commands]
    outputs = [path if path.is_absolute() else directory / path for path in outputs]
    locks = [path if path.is_absolute() else directory / path for path in locks]
    installed = [path if path.is_absolute() else directory / path for path in installed]
    stdout = {index: path if path.is_absolute() else directory / path for index, path in (stdout or {}).items()}
    capture = _snapshot(directory, profile, parents, inputs)
    lock_inputs = {str(path.resolve()): _source(path) for path in locks}
    installed_inputs = {str(path.resolve()): _installed(path, project) for path in installed}
    command_tools = _command_tools(directory, commands)
    origins = {digest for parent in parents for digest in _outputs(parent).values()}
    for command in commands:
        executable = Path(command[0]).name.lower().removesuffix(".exe")
        if role == "frontend" and (executable != "node" or "build" not in command):
            raise policy.TestError("frontend production requires the authored Node build invocation")
        if role == "python-package":
            prepared = {"--no-build-isolation", "--offline", "--python"}.issubset(command)
            if executable != "uv" or command[1:2] != ["build"] or not prepared or not installed:
                raise policy.TestError("Python package production requires offline, nonisolated uv build, an explicit Python and captured backend dependencies")
        if role == "rust-package" and (executable != "cargo" or command[1:2] != ["package"] or not {"--locked", "--no-verify"}.issubset(command)):
            raise policy.TestError("Rust package production requires cargo package --locked --no-verify without compilation")
        if role == "dependency-export":
            cargo = executable == "cargo" and command[1:2] == ["vendor"] and "--locked" in command
            uv = executable == "uv" and command[1:2] == ["export"] and "--locked" in command
            pip = Path(command_tools[command[0]]["path"]).resolve() == Path(sys.executable).resolve() and "download" in command and "--require-hashes" in command
            if not (cargo or uv or pip):
                raise policy.TestError("dependency exports require locked native Cargo/uv or hash-checked pip invocations")
        if role == "package" and command_tools[command[0]]["sha256"] not in origins:
            raise policy.TestError("native packaging tool must come from a verified original parent compiler output")
    for output in outputs:
        _inside(project, output.relative_to(project).as_posix())
        output = output.resolve()
        if not output.is_relative_to(project) or output == project or output.is_symlink():
            raise policy.TestError(f"native output must stay in the owned project: {output}")
        if output.exists() and (not output.is_dir() or any(output.iterdir())):
            raise policy.TestError(f"native output is not fresh; prepare a new owned output location: {output}")
    for index, command in enumerate(commands):
        destination = (stdout or {}).get(index)
        if destination is None:
            status = _run(command, directory)
        else:
            if not any(destination.resolve() == output.resolve() or destination.resolve().is_relative_to(output.resolve()) for output in outputs):
                raise policy.TestError("native stdout must be a declared output")
            destination.parent.mkdir(parents=True, exist_ok=True)
            with destination.open("w", encoding="utf-8") as stream:
                status = _run(command, directory, stream)
        if status != 0:
            raise policy.TestError(f"native {role} production failed")
    exports = {}
    for output in outputs:
        _inside(project, output.relative_to(project).as_posix())
        if output.is_file():
            exports[str(output.resolve())] = _source(output)
        elif output.is_dir():
            for path in output.rglob("*"):
                if path.is_symlink():
                    raise policy.TestError(f"unbounded symlink in native output: {path}")
                if path.is_file():
                    exports[str(path.resolve())] = _digest(path)
        else:
            raise policy.TestError(f"native producer did not write its declared output: {output}")
    if not exports:
        raise policy.TestError("native producer emitted no files")
    _unchanged(directory, profile, capture)
    if any(_source(Path(name)) != digest for name, digest in lock_inputs.items()):
        raise policy.TestError("native dependency lock changed during production")
    if any(_installed(Path(name), project) != digest for name, digest in installed_inputs.items()):
        raise policy.TestError("installed dependency inputs changed during production")
    if _command_tools(directory, commands) != command_tools:
        raise policy.TestError("native producer executable changed during production")
    key = hashlib.sha256(json.dumps([role, [str(path) for path in outputs]]).encode()).hexdigest()[:16]
    receipt = project / "target/producers" / role / key / profile / "production.json"
    record = {"schema": 1, "kind": role, "workspace": directory.relative_to(project).as_posix(), "receipt": _relative(project, receipt),
              "profile": profile, "arguments": [], **capture, "resolution": {"locks": lock_inputs}, "installed_inputs": installed_inputs,
              "commands": [list(command) for command in commands], "command": list(commands[0]), "command_tools": command_tools,
              "exports": exports, "provider": _provider()}
    receipt.parent.mkdir(parents=True, exist_ok=True)
    receipt.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return record


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
    if record.get("kind") != "nextest":
        raise policy.TestError("nextest archive has no original archive producer receipt")
    _verify_inputs(directory, record, profile, current, arguments)
    project = Path(_git(directory, "rev-parse", "--show-toplevel"))
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
    if record.get("schema") != 1 or not _outputs(record):
        raise policy.TestError("no complete producer output map; rebuild the artifacts")
    expected = _outputs(record)
    selected = list(expected) if paths is None else [str(path) for path in paths]
    for name in selected:
        if name not in expected:
            raise policy.TestError(f"output was not recorded by the compiler producer: {name}")
        if _source(Path(name)) != expected[name]:
            raise policy.TestError(f"output changed after compiler production: {name}")


def verify_copy(record: dict, source: Path, destination: Path) -> str:
    """Verify source and copied bytes against the immutable producer map; return its original digest."""
    verify_outputs(record, [source])
    expected = _outputs(record)[str(source)]
    if _source(destination) != expected:
        raise policy.TestError(f"staged copy differs from original compiler output: {destination}")
    return expected


def stamp(directory: Path, producers: Sequence[dict], files: Mapping[str, str], metadata: dict | None = None) -> dict:
    """Bind staged files to unchanged original captures and caller-supplied immutable expected digests.

    Expected hashes come from verified producer maps or typed package/export receipts, never
    by hashing the staged copies to declare them valid. Product selectors belong in metadata.
    """
    if not producers or not files:
        raise policy.TestError("a fixture stamp requires original producer captures and expected files")
    current = tree(directory)
    profile = producers[0].get("profile")
    for producer in producers:
        workspace = _inside(directory, producer.get("workspace", "."))
        _verify_inputs(workspace, producer, profile, current, producer.get("arguments", []))
    origins = {digest for producer in producers for digest in _outputs(producer).values()}
    if any(digest not in origins for digest in files.values()):
        raise policy.TestError("fixture files require an original compiler/generated/native-export digest")
    _verify_files(directory, files)
    record = {"schema": 1, "tree": current, "revision": producers[0]["revision"], "profile": profile,
              "producers": list(producers), "files": dict(files), "metadata": metadata or {}}
    destination = _inside(directory, FIXTURES)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return record


def fixtures(directory: Path, current: str, profile: str = "dev", hosts: Sequence[str] = ()) -> None:
    """Verify original producer identity and every expected staged byte before consuming fixtures."""
    path = _inside(directory, FIXTURES)
    if not path.is_file():
        if hosts:
            raise policy.TestError(f"{FIXTURES} is absent; build the declared host fixtures")
        return
    record = json.loads(path.read_text(encoding="utf-8"))
    if record.get("schema") != 1 or record.get("tree") != current or record.get("profile") != profile:
        raise policy.TestError(f"{FIXTURES} belongs to another source tree or profile; rebuild the fixtures")
    if record.get("revision") != revision(directory) or not record.get("producers"):
        raise policy.TestError(f"{FIXTURES} has no current original producer identity")
    for producer in record["producers"]:
        workspace = _inside(directory, producer.get("workspace", "."))
        _verify_inputs(workspace, producer, profile, current, producer.get("arguments", []))
    files = record.get("files", {})
    origins = {digest for producer in record["producers"] for digest in _outputs(producer).values()}
    if any(digest not in origins for digest in files.values()):
        raise policy.TestError("fixture digest has no original producer output")
    if not files:
        raise policy.TestError(f"{FIXTURES} has no expected artifact files")
    _verify_files(directory, files)
    machine = {"amd64": "x86_64", "arm64": "aarch64"}.get(platform.machine().lower(), platform.machine().lower())
    system = {"linux": "linux", "win32": "windows", "darwin": "macos"}.get(sys.platform, sys.platform)
    target = record["producers"][0].get("target", "")
    for host in hosts:
        name = host.format(profile=profile, platform=system, architecture=machine, target=target, exe=".exe" if os.name == "nt" else "")
        artifact = _inside(directory, name)
        if artifact.is_file():
            if name not in files:
                raise policy.TestError(f"host has no immutable fixture digest: {name}")
        elif artifact.is_dir():
            expected = {file for file in files if Path(file).is_relative_to(name)}
            actual = {file.relative_to(directory).as_posix() for file in artifact.rglob("*") if file.is_file() or file.is_symlink()}
            if not expected or actual != expected:
                raise policy.TestError(f"host directory differs from its expected fixture files: {name}")
        else:
            raise policy.TestError(f"missing host artifact: {name}")


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


def _outputs(record: dict) -> dict[str, str]:
    outputs = {**record.get("artifacts", {}), **record.get("generated", {}), **record.get("exports", {})}
    if record.get("project") and record.get("receipt"):
        path = Path(record["project"]) / record["receipt"]
        original = (json.dumps(record, indent=2) + "\n").encode()
        outputs[str(path)] = hashlib.sha256(original).hexdigest()
    return outputs


def _provider() -> dict:
    return {key: os.environ[key] for key in ("GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_JOB", "CI_JOB_ID", "CI_PIPELINE_ID") if key in os.environ}


def _snapshot(directory: Path, profile: str, parents: Sequence[dict], inputs: Sequence[Path]) -> dict:
    if profile not in (*policy.PROFILES, "bench"):
        raise policy.TestError("producer profile must be dev, release or bench")
    project = Path(_git(directory, "rev-parse", "--show-toplevel"))
    sources = {project / name for name in _git(project, "ls-files", "--cached", "--others", "--exclude-standard", "-z").split("\0") if name}
    if linked := next((file for file in sources if file.is_symlink()), None):
        raise policy.TestError(f"unbounded symbolic link in workspace source: {linked}")
    current = tree(project)
    record = {"project": str(project), "tree": current, "revision": revision(project), "inputs": _inputs(directory), "parents": list(parents), "parent_inputs": {}, "declared_inputs": {}, "input_directories": {}}
    origins = {digest for parent in parents for digest in _outputs(parent).values()}
    for input in inputs:
        input = input if input.is_absolute() else directory / input
        _unlinked(input)
        paths = [input] if input.is_file() else [path for path in input.rglob("*") if path.is_file() or path.is_symlink()]
        if not paths:
            raise policy.TestError(f"declared producer input is missing or empty: {input}")
        if input.is_dir():
            record["input_directories"][str(input.resolve())] = sorted(str(path.resolve()) for path in paths)
        for path in paths:
            digest = _source(path)
            group = "parent_inputs" if digest in origins else "declared_inputs"
            record[group][str(path.resolve())] = digest
    _verify_parents(directory, record, profile, current)
    return record


def _unchanged(directory: Path, profile: str, record: dict) -> None:
    if tree(directory) != record["tree"] or revision(directory) != record["revision"] or _inputs(directory) != record["inputs"]:
        raise policy.TestError("source, revision or effective tool/configuration inputs changed during production")
    _verify_directories(record)
    for group in ("parent_inputs", "declared_inputs"):
        if any(_source(Path(name)) != digest for name, digest in record[group].items()):
            raise policy.TestError("a declared/generated producer input changed during production")
    _verify_parents(directory, record, profile, record["tree"])


def _verify_directories(record: dict, *, missing_parents: bool = False) -> None:
    for name, expected in record.get("input_directories", {}).items():
        directory = Path(name)
        _unlinked(directory)
        if missing_parents and not directory.exists() and expected and all(path in record.get("parent_inputs", {}) for path in expected):
            continue
        actual = sorted(str(path.resolve()) for path in directory.rglob("*") if path.is_file() or path.is_symlink())
        if directory.is_symlink() or actual != expected:
            raise policy.TestError(f"declared producer input directory membership changed: {name}")


def _verify_parents(directory: Path, record: dict, profile: str, current: str) -> None:
    project = Path(_git(directory, "rev-parse", "--show-toplevel"))
    origins = set()
    for parent in record.get("parents", []):
        workspace = _inside(project, parent.get("workspace", "."))
        _verify_inputs(workspace, parent, profile, current, parent.get("arguments", []))
        origins.update(_outputs(parent).values())
    for name, digest in record.get("parent_inputs", {}).items():
        if digest not in origins:
            raise policy.TestError(f"generated input has no verified original parent output: {name}")
        if Path(name).exists() and _source(Path(name)) != digest:
            raise policy.TestError(f"present generated input differs from its original parent: {name}")


def _command_tools(directory: Path, commands: Sequence[Sequence[str]]) -> dict:
    tools = {}
    for command in commands:
        if not command:
            raise policy.TestError("native invocation is empty")
        names = [command[0]]
        if "--python" in command:
            index = command.index("--python") + 1
            if index == len(command):
                raise policy.TestError("native Python selection requires an executable path")
            names.append(command[index])
        for name in names:
            path = Path(name)
            executable = directory / path if not path.is_absolute() and len(path.parts) > 1 else Path(shutil.which(name) or name)
            executable = executable.resolve()
            tools[name] = {"path": str(executable), "sha256": _digest(executable)}
    return tools


def _installed(path: Path, boundary: Path) -> str:
    entries, visited = [], set()
    pending = [path]
    while pending:
        file = pending.pop()
        resolved = file.resolve()
        if not resolved.is_relative_to(boundary):
            raise policy.TestError(f"installed dependency escapes its declared project boundary: {file}")
        if file.is_symlink():
            entries.append((str(file), "link", str(resolved)))
        if resolved in visited:
            continue
        visited.add(resolved)
        if resolved.is_file():
            entries.append((str(resolved), "file", _digest(resolved)))
        elif resolved.is_dir():
            pending.extend(resolved.iterdir())
        else:
            raise policy.TestError(f"installed dependency input is missing: {file}")
    return hashlib.sha256(json.dumps(sorted(entries)).encode()).hexdigest()


def _verify_inputs(directory: Path, record: dict, profile: str, current: str, arguments: Sequence[str]) -> None:
    if record.get("tree") != current or record.get("profile") != profile:
        raise policy.TestError("producer capture belongs to another source tree or profile")
    if record.get("schema") != 1 or "inputs" not in record:
        raise policy.TestError("producer capture has no complete input receipt; rebuild it")
    if record.get("arguments") != list(arguments):
        raise policy.TestError("producer capture uses another Cargo argument selection")
    if record.get("revision") != revision(directory):
        raise policy.TestError("producer capture belongs to another source revision")
    if record.get("inputs") != _inputs(directory):
        raise policy.TestError("producer capture uses another platform, environment or configuration")
    project = Path(_git(directory, "rev-parse", "--show-toplevel"))
    _verify_parents(directory, record, profile, current)
    _verify_directories(record, missing_parents=True)
    for name, digest in record.get("declared_inputs", {}).items():
        if _source(Path(name)) != digest:
            raise policy.TestError(f"declared producer input changed: {name}")
    resolution = record.get("resolution", {})
    if record.get("kind") in ("dependency-export", "frontend", "python-package", "rust-package", "package"):
        if not resolution.get("locks") or any(_source(Path(name)) != digest for name, digest in resolution["locks"].items()):
            raise policy.TestError("native producer lock inputs changed")
        for name, digest in record.get("installed_inputs", {}).items():
            if Path(name).exists() and _installed(Path(name), project) != digest:
                raise policy.TestError(f"installed native dependency changed: {name}")
        origins = {name: digest for parent in record.get("parents", []) for name, digest in _outputs(parent).items()}
        for tool in record["command_tools"].values():
            path = Path(tool["path"])
            if not path.exists() and origins.get(str(path)) == tool["sha256"]:
                continue
            if _source(path) != tool["sha256"]:
                raise policy.TestError("native producer executable identity changed")
        return
    lock = project / resolution.get("lock", "Cargo.lock")
    if not resolution.get("packages") or resolution.get("sha256") != _digest(lock):
        raise policy.TestError("producer capture uses another locked dependency resolution")
    if "watched" not in record or record.get("watched_inputs") != _watched_inputs(record["watched"]):
        raise policy.TestError("producer capture build-script inputs changed")
    for name, digest in record.get("dependencies", {}).items():
        if _source(Path(name)) != digest:
            raise policy.TestError(f"producer capture dependency changed: {name}")


def _inside(directory: Path, name: str) -> Path:
    relative = Path(name)
    if relative.is_absolute() or ".." in relative.parts or "\\" in name:
        raise policy.TestError(f"fixture path must stay relative to the project: {name}")
    root = directory.resolve()
    path = root / relative
    current = root
    for part in relative.parts:
        current = current / part
        if current.is_symlink():
            raise policy.TestError(f"fixture path contains an unbounded symlink: {name}")
    if not path.resolve().is_relative_to(root):
        raise policy.TestError(f"fixture path escapes the project: {name}")
    return path


def _verify_files(directory: Path, files: Mapping[str, str]) -> None:
    for name, expected in files.items():
        if name == FIXTURES or not isinstance(expected, str) or len(expected) != 64 or any(character not in "0123456789abcdef" for character in expected):
            raise policy.TestError(f"fixture requires an immutable expected SHA-256: {name}")
        if _source(_inside(directory, name)) != expected:
            raise policy.TestError(f"staged fixture differs from its original expected bytes: {name}")


def _produce(directory: Path, profile: str, extra: Sequence[str], *, archived: bool, parents: Sequence[dict], inputs: Sequence[Path]) -> dict:
    if profile not in (*policy.PROFILES, "bench"):
        raise policy.TestError("build profile must be dev, release or bench")
    if any(argument in ("--config", "--manifest-path") or argument.startswith(("--config=", "--manifest-path=")) for argument in extra):
        raise policy.TestError("build inputs require repository-owned Cargo configuration and the current workspace manifest")
    directory = directory.resolve()
    project = Path(_git(directory, "rev-parse", "--show-toplevel"))
    capture = _snapshot(directory, profile, parents, inputs)
    before = capture["tree"]
    build_inputs = capture["inputs"]
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
    if _inputs(directory) != build_inputs or revision(project) != original:
        raise policy.TestError("the build environment or source revision changed while the archive was built")

    _unchanged(directory, profile, capture)

    # Edits During Compilation Make The Archive Unattributable.
    if tree(project) != before:
        raise policy.TestError("the source tree changed while the archive was built")
    record = {
        "schema": 1,
        "kind": "nextest" if archived else "cargo",
        "project": str(project),
        "workspace": directory.relative_to(project).as_posix(),
        "receipt": _relative(project, receipt),
        "tree": before,
        "revision": original,
        "published_revision": published(project, before),
        "provider": {key: os.environ[key] for key in ("GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_JOB", "CI_JOB_ID", "CI_PIPELINE_ID") if key in os.environ},
        "inputs": build_inputs,
        "parents": capture["parents"],
        "parent_inputs": capture["parent_inputs"],
        "declared_inputs": capture["declared_inputs"],
        "input_directories": capture["input_directories"],
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
            "rustc": _toolchain(directory) if shutil.which("rustc") else None,
            "cargo": _output(directory, ["cargo", "-V"]).strip() if shutil.which("cargo") else None,
            "runtimes": {name: _output(directory, [name, "--version"]).strip() for name in ("node", "pnpm", "uv", "python") if shutil.which(name)}, "tools": _tools(directory)}


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
    selected = {name: name for name in ("rustc", "cargo", "cc", "c++", "ld", "ar", "cl", "link", "pkg-config", "node", "pnpm", "uv", "python")}
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
        if executable is None and name not in ("rustc", "cargo", "cc", "c++", "ld", "ar", "cl", "link", "pkg-config", "node", "pnpm", "uv", "python"):
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


def _unlinked(path: Path) -> None:
    for ancestor in (path, *path.parents):
        if ancestor.is_symlink():
            raise policy.TestError(f"unbounded symlink in declared build input: {ancestor}")


def _source(path: Path) -> str:
    _unlinked(path)
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
