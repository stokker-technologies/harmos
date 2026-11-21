# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Plan Changed-Path CI Jobs"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import re
import subprocess
from collections.abc import Iterable, Mapping
from pathlib import Path, PurePosixPath
from typing import Any

# Third Party Imports
# Local Imports
from ..process import spawn_flags
from .matrices import CHECKS, TESTS

# Type Checking Imports

# === Public Exports === #

# === Constants === #

JOBS = (
    "docs",
    "check",
    "build_product",
    "build_tests",
    "check_product",
    "package_rust",
    "prepare_test_fixtures",
    "test_unit",
    "test",
    "release_dry",
    "release_rehearsal",
)
DEVELOPMENT = {"docs", "check", "build_product"}
RELEASE = {"release_dry", "release_rehearsal"}
CLASSES = {"docs", "rust", "rust_tests", "python", "csharp", "cpp", "package", "all"}
PRODUCERS = {
    "check_product": {"build_product"},
    "package_rust": {"build_product"},
    "prepare_test_fixtures": {"build_product", "build_tests"},
    "test_unit": {"build_tests"},
    "test": {"prepare_test_fixtures"},
    "release_dry": {"package_rust"},
    "release_rehearsal": {"package_rust"},
}

# === Types === #

# === Classes === #

# === Public Functions === #


def classify(paths: Iterable[str] | None) -> frozenset[str]:
    """Classify Both Rename Sides and Deleted Paths Without Reading the Filesystem."""
    if paths is None:
        return frozenset({"all"})
    classes: set[str] = set()
    for path in paths:
        parts = PurePosixPath(path).parts
        if not path or path.startswith("/") or "\\" in path or any(part in {".", ".."} for part in parts):
            return frozenset({"all"})
        if path.startswith(("docs/", "openspec/", "sites/docs/", "tools/python/src/harmos_tools/docs/")) or path.endswith(".md"):
            classes.add("docs")
        elif (
            path.startswith((".github/", "proto/", "tools/", ".cargo/"))
            or path
            in {
                "mise.toml",
                "mise.lock",
                "Cargo.toml",
                "Cargo.lock",
                "pyproject.toml",
                "uv.lock",
                "rust-toolchain",
                "rust-toolchain.toml",
                "buf.yaml",
                "buf.lock",
                "buf.gen.yaml",
            }
            or "descriptor" in path
        ):
            classes.add("all")
        elif path.startswith("sdk/python/"):
            classes.add("python")
        elif path.startswith("sdk/csharp/"):
            classes.add("csharp")
        elif path.startswith("sdk/cpp/"):
            classes.add("cpp")
        elif path.startswith("sdk/tests/"):
            classes.add("all")
        elif path.startswith(("crates/", "sdk/rust/", "examples/")):
            classes.add("rust_tests" if "/tests/" in path or "/benches/" in path else "rust")
        elif path.startswith(("packaging/", "installers/")) or path in {"LICENSE", "LICENSE-APACHE", "LICENSE-MIT"}:
            classes.add("package")
        else:
            classes.add("all")
    return frozenset(classes or {"all"})


def plan(classes: frozenset[str], event: str) -> dict[str, Any]:
    """Union Consumers, Close Artifact Dependencies, Then Apply Event Policy."""
    if event not in {"review", "push", "release", "nightly", "full", "draft", "closed"}:
        raise ValueError("Plan CI Impact: Choose a Supported Event")
    full = event in {"release", "nightly", "full"} or "all" in classes or not classes or not classes <= CLASSES
    languages = {"rust", "python", "csharp", "cpp"} if full else set()
    selected: set[str] = set(JOBS) if full else set()
    if "docs" in classes:
        selected.add("docs")
    if "rust" in classes:
        languages.update({"rust", "python", "csharp", "cpp"})
        selected.update(set(JOBS) - RELEASE)
    if "rust_tests" in classes:
        languages.add("rust")
        selected.update({"check", "test", "test_unit"})
    if "python" in classes:
        languages.update({"python", "rust"})
        selected.update({"docs", "check", "check_product", "test"})
    if "csharp" in classes:
        languages.add("csharp")
        selected.update({"docs", "check", "check_product", "test"})
    if "cpp" in classes:
        languages.update({"cpp", "python"})
        selected.update({"docs", "check", "check_product", "test"})
    if "package" in classes:
        selected.update({"check", "package_rust"})
    checks = [dict(row) for row in CHECKS if full or row["language"] in languages or row["language"] == "repository"]
    tests = [dict(row) for row in TESTS]
    while True:
        expanded = selected | set().union(*(PRODUCERS.get(job, set()) for job in selected))
        if expanded == selected:
            break
        selected = expanded
    if event == "push":
        selected &= DEVELOPMENT
    elif event not in {"release", "nightly", "full"}:
        selected -= RELEASE
    if event in {"draft", "closed"}:
        selected.clear()
    return {
        **{job: job in selected for job in JOBS},
        "selected": sorted(selected),
        "skipped": sorted(set(JOBS) - selected),
        "check_matrix": {"include": checks},
        "test_matrix": {"include": tests},
    }


def diff_paths(data: bytes) -> list[str]:
    """Decode Git's NUL-Delimited Name-Status Format, Rejecting Partial Records."""
    if not data or not data.endswith(b"\0"):
        raise ValueError("Plan CI Impact: Diff Is Missing or Truncated; Select Every Job")
    fields = data[:-1].decode("utf-8").split("\0")
    paths = []
    index = 0
    while index < len(fields):
        status = fields[index]
        if re.fullmatch(r"[ACDMRTUXB][0-9]*", status) is None:
            raise ValueError("Plan CI Impact: Invalid Diff Status; Select Every Job")
        count = 2 if status.startswith(("R", "C")) else 1
        names = fields[index + 1 : index + 1 + count]
        if len(names) != count or not all(names):
            raise ValueError("Plan CI Impact: Partial Diff Record; Select Every Job")
        paths.extend(names)
        index += count + 1
    return paths


def changed_paths(event: Mapping[str, Any], name: str, directory: Path) -> list[str] | None:
    """Use Merge-Base to PR Head or Push Before to After; Fail Open on Diff Errors."""
    try:
        if name == "pull_request":
            base = event["pull_request"]["base"]["sha"]
            head = event["pull_request"]["head"]["sha"]
        elif name == "push":
            base, head = event["before"], event["after"]
        else:
            return None
        for revision in (base, head):
            if not isinstance(revision, str) or re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", revision) is None or set(revision) == {"0"}:
                return None
            _git(directory, "cat-file", "-e", revision + "^{commit}")
        if name == "pull_request":
            base = _git(directory, "merge-base", base, head).decode("ascii").strip()
        data = _git(directory, "diff", "--no-ext-diff", "--no-textconv", "--name-status", "-z", "--find-renames", base, head, "--")
        return diff_paths(data)
    except (KeyError, TypeError, OSError, UnicodeError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Plan CI Impact: Complete Comparison Unavailable; Selecting Every Eligible Job ({error})")
        return None


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", choices=("review", "push", "release", "nightly", "full", "draft", "closed"))
    options = parser.parse_args(arguments)
    event = {}
    if event_path := os.environ.get("GITHUB_EVENT_PATH"):
        try:
            event = json.loads(Path(event_path).read_text(encoding="utf-8"))
            if not isinstance(event, dict):
                event = {}
        except (OSError, ValueError):
            pass
    name = os.environ.get("GITHUB_EVENT_NAME", "")
    selected_event = options.event or _event(name, event)
    paths = changed_paths(event, name, Path.cwd()) if selected_event in {"review", "push"} else None
    result = plan(classify(paths), selected_event)
    print(json.dumps({key: result[key] for key in ("selected", "skipped")}, sort_keys=True))
    if destination := os.environ.get("GITHUB_OUTPUT"):
        output = "".join(f"{key}={json.dumps(value, separators=(',', ':'))}\n" for key, value in result.items())
        destination = Path(destination)
        previous = destination.read_text(encoding="utf-8") if destination.exists() else ""
        destination.write_text(previous + output, encoding="utf-8", newline="\n")
    return 0


# === Private Functions === #


def _git(directory: Path, *arguments: str) -> bytes:
    result = subprocess.run(["git", *arguments], cwd=directory, check=True, capture_output=True, creationflags=spawn_flags())
    return result.stdout


def _event(name: str, event: Mapping[str, Any]) -> str:
    if name == "push":
        return "push"
    if name == "pull_request":
        if event.get("action") == "closed":
            return "closed"
        pull = event.get("pull_request", {})
        if pull.get("draft"):
            return "draft"
        if pull.get("head", {}).get("ref", "").startswith("release/v"):
            return "release"
        return "review"
    return "full"


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
