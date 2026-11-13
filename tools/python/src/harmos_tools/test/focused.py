# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build Only Selected Rust Test Targets And Run Them With Native Hang Guards."""

# === Imports === #

# Standard Library Imports
import json
import os
import re
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import Any
from xml.etree import ElementTree

# Third Party Imports

# Local Imports
from .. import temporary
from ..process import spawn_flags
from . import nextest, policy, report

# Type Checking Imports

# === Public Exports === #

# === Constants === #

FIXTURES = Path("crates/harmos-runtime/tests/support/fixtures.rs")

# === Types === #

# === Classes === #

# === Public Functions === #


def select(directory: Path, metadata: dict[str, Any], paths: Sequence[Path]) -> dict[str, list[str]]:
    """Select Test Targets, Including Local Consumers Of Changed Product Code."""
    packages = {item["name"]: item for item in metadata["packages"] if item["id"] in metadata["workspace_members"]}
    roots = {name: Path(item["manifest_path"]).parent.resolve() for name, item in packages.items()}
    selected: dict[str, set[str]] = {}
    sources: dict[Path, set[Path]] = {}
    affected = set()
    for changed in paths:
        path = (directory / changed).resolve()
        if not path.is_relative_to(directory.resolve()):
            raise policy.TestError(f"changed path escapes the repository: {changed}")
        global_input = path.name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "mise.toml") and path.parent == directory.resolve()
        if global_input or path.is_relative_to(directory / "proto"):
            affected.update(packages)
            continue
        owners = [name for name, root in roots.items() if path.is_relative_to(root)]
        if not owners:
            raise policy.TestError(f"no Rust package owns {changed}; select its language and test explicitly")
        owner = max(owners, key=lambda name: len(roots[name].parts))
        for folder in (path, *path.parents):
            if folder == roots[owner]:
                break
            if (folder / "Cargo.toml").is_file():
                raise policy.TestError(
                    f"{changed} belongs to a separate Cargo workspace; select its --manifest-path and --package explicitly"
                )
        if path.is_relative_to(roots[owner] / "tests"):
            # Shared Test Support Can Be Included By Another Package's Test Target.
            found = False
            for name, item in packages.items():
                for target in item["targets"]:
                    if "test" not in target["kind"]:
                        continue
                    entry = Path(target["src_path"])
                    if entry not in sources:
                        sources[entry] = _sources(entry)
                    if path in sources[entry]:
                        selected.setdefault(name, set()).add(target["name"])
                        found = True
            if not found:
                selected.setdefault(owner, set()).update(_targets(packages[owner]))
        else:
            affected.add(owner)

    # Product Changes Also Affect Tests In Their Local Dependency Closure.
    while True:
        consumers = {
            name
            for name, item in packages.items()
            if any(dependency.get("path") and dependency["name"] in affected for dependency in item["dependencies"])
        }
        expanded = affected | consumers
        if expanded == affected:
            break
        affected = expanded
    for name in affected:
        selected.setdefault(name, set()).update(_targets(packages[name]))
    if affected:
        # Prepared Libraries And Components Are Dependencies Outside Cargo's Graph.
        helper = (directory / FIXTURES).resolve()
        for name, item in packages.items():
            for target in item["targets"]:
                if "test" not in target["kind"]:
                    continue
                entry = Path(target["src_path"])
                if entry not in sources:
                    sources[entry] = _sources(entry)
                if helper in sources[entry]:
                    selected.setdefault(name, set()).add(target["name"])
    return {name: sorted(targets) for name, targets in sorted(selected.items()) if targets}


def run(
    directory: Path,
    *,
    package: str | None = None,
    target: str | None = None,
    paths: Sequence[Path] = (),
    manifest: Path | None = None,
    features: str | None = None,
    exact: str | None = None,
    story: str | None = None,
    no_run: bool = False,
    timings: bool = False,
) -> int:
    selectors = ["--manifest-path", str(manifest)] if manifest else []
    selected: dict[str, list[str]]
    if target is not None and package is not None:
        selected = {package: [target]}
    else:
        result = subprocess.run(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1", *selectors],
            cwd=directory,
            capture_output=True,
            text=True,
            encoding="utf-8",
            creationflags=spawn_flags(),
            check=False,
        )
        if result.stderr:
            print(result.stderr, file=sys.stderr, end="")
        if result.returncode:
            return result.returncode
        metadata = json.loads(result.stdout)
        if paths:
            selected = select(directory, metadata, paths)
        else:
            candidates = {item["name"]: item for item in metadata["packages"]}
            if package is None or package not in candidates:
                raise policy.TestError(f"unknown Rust package: {package}")
            selected = {package: _targets(candidates[package])}
    if not selected or not any(selected.values()):
        raise policy.TestError("selection contains no Rust test targets")

    rules = policy.load(directory)
    names = sorted({name for targets in selected.values() for name in targets if name != "lib"})
    overrides = nextest.overrides(rules, None, coverage=False, binaries={name: name for name in names})
    composite = target is None
    executed = 0
    output = report.native_directory(directory, Path("target/tests/focused"))
    with temporary.directory() as scratch:
        configuration = scratch / "nextest.toml"
        for name, targets in selected.items():
            # System Stories Retain Their Existing Serial Scheduling.
            for system in (False, True):
                group = [item for item in targets if (item == "system" or item.startswith("system-")) == system]
                if not group:
                    continue
                store = report.native_directory(output, Path(name) / ("system" if system else "local"))
                junit = store / nextest.PROFILE / "junit.xml"
                junit.unlink(missing_ok=True)
                configuration.write_text(nextest.config(str(store), max(rules.deadlines.values()), overrides), encoding="utf-8")
                command = ["cargo", "nextest", "run", "--locked", "-p", name, *selectors]
                for item in group:
                    if item == "lib":
                        command += ["--lib"]
                    elif item.startswith("bin/"):
                        command += ["--bin", item.removeprefix("bin/")]
                    else:
                        command += ["--test", item]
                if features:
                    command += ["--features", features]
                command += [
                    "--config-file",
                    str(configuration),
                    "--profile",
                    nextest.PROFILE,
                    "--no-tests",
                    "warn" if composite else "fail",
                ]
                if system:
                    command += ["--test-threads", "1"]
                if no_run:
                    command.append("--no-run")
                if timings:
                    command.append("--timings")
                if story:
                    prefix = re.escape(story.removesuffix("::") + "::").replace("/", r"\/")
                    command += ["-E", f"test(/^{prefix}/)"]
                if exact:
                    command += ["--", exact, "--exact"]
                print(f"Focused Tests: {name} / {', '.join(group)}", flush=True)
                environment = dict(os.environ, CARGO_BUILD_WARNINGS="deny", CARGO_TARGET_DIR=str(directory / "target"))
                suffix = ".exe" if os.name == "nt" else ""
                environment.setdefault("HARMOS_CPP_PLUGIN", str(directory / "target/cpp/tests" / f"harmos-cpp-conformance{suffix}"))
                result = subprocess.run(
                    command,
                    cwd=directory,
                    env=environment,
                    stdin=sys.stdin,
                    stdout=sys.stdout,
                    stderr=sys.stderr,
                    creationflags=spawn_flags(),
                    check=False,
                )
                if result.returncode:
                    return result.returncode
                if not no_run:
                    if not junit.is_file():
                        raise policy.TestError(f"focused tests produced no native report: {junit}")
                    executed += sum(case.find("skipped") is None for case in ElementTree.parse(junit).iter("testcase"))
    if not no_run and executed == 0:
        raise policy.TestError("selection executed no tests")
    return 0


# === Private Functions === #


def _targets(package: dict[str, Any]) -> list[str]:
    selected = []
    for target in package["targets"]:
        if "test" in target["kind"]:
            selected.append(target["name"])
        elif target.get("test") and any(kind in ("lib", "rlib", "proc-macro") for kind in target["kind"]):
            selected.append("lib")
        elif target.get("test") and "bin" in target["kind"]:
            selected.append("bin/" + target["name"])
    return sorted(selected)


def _sources(entry: Path) -> set[Path]:
    """Follow Authored Out-Of-Line Modules And Included Fixtures Conservatively."""
    found = set()
    pending = [entry.resolve()]
    while pending:
        path = pending.pop()
        if path in found:
            continue
        found.add(path)
        if not path.is_file() or path.suffix != ".rs":
            continue
        content = path.read_text(encoding="utf-8")
        base = path.parent if path.name in ("main.rs", "lib.rs", "mod.rs") else path.with_suffix("")
        for attributes, module in re.findall(r"((?:#\[[^\]]*\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;", content):
            override = re.search(r'#\[path\s*=\s*"([^"]+)"\]', attributes)
            if override:
                pending.append((path.parent / override[1]).resolve())
            else:
                # A Path-Attributed Parent Can Resolve Children Beside Its Source.
                for folder in {base, path.parent}:
                    pending.extend(candidate.resolve() for candidate in (folder / f"{module}.rs", folder / module / "mod.rs"))
        for included in re.findall(r'include(?:_str|_bytes)?!\(\s*"([^"]+)"\s*\)', content):
            pending.append((path.parent / included).resolve())
    return found


# === Entry Point === #
