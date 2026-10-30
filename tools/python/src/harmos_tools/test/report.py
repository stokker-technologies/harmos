# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Read And Preserve Native JUnit Reports."""

# === Imports === #

# Standard Library Imports
import json
import os
import sys
from collections.abc import Generator, Iterator, Sequence
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from xml.etree import ElementTree

# Third Party Imports
# Local Imports
from ..build import paths
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

TESTS = Path("target/tests")
BENCHMARKS = Path("target/benchmarks")
TRACES = Path("target/traces")
LOCK = TESTS / ".lock"

# === Types === #

# === Classes === #


@dataclass(frozen=True)
class Case:
    """One story of a JUnit report; `suite` and `name` are its native identity."""

    category: str
    language: str
    package: str
    suite: str
    name: str
    outcome: str
    seconds: float

    @property
    def file(self) -> str:
        if self.language != "rust":
            return self.suite
        module = self.name.rpartition("::")[0]
        binary = self.suite.partition("::")[2] or "lib"
        return f"{binary}::{module}" if module else binary

    @property
    def story(self) -> str:
        return self.name.rpartition("::")[2] if self.language == "rust" else self.name


# === Public Functions === #


def native_directory(root: Path, relative: Path) -> Path:
    path = root / relative
    if not path.resolve().is_relative_to(root.resolve()):
        raise policy.TestError("native report directory escapes the repository")
    if any(item.is_symlink() or item.is_junction() for item in (path, *path.parents) if item.is_relative_to(root)):
        raise policy.TestError("native report directory must not contain links")
    path.mkdir(parents=True, exist_ok=True)
    return path.resolve()


def location(directory: Path, profile: str, key: str) -> Path:
    parts = key.split("-")
    if parts[-1] not in policy.LANGUAGES:
        raise policy.TestError("unknown report language")
    if parts[0] == "coverage" and len(parts) in (2, 3):
        if len(parts) == 3 and parts[1] not in policy.CATEGORIES:
            raise policy.TestError("unknown coverage layer")
        return paths.output(directory, "tests", profile).joinpath("coverage", parts[-1], *parts[1:-1])
    if len(parts) != 2 or parts[0] not in policy.CATEGORIES:
        raise policy.TestError("unknown report layer")
    return paths.output(directory, "tests", profile) / key


def cases(text: str, category: str, language: str, package: str) -> list[Case]:
    """The stories of one JUnit document; Rust packages come from the nextest binary id."""
    found = []
    for suite, node in _testcases(_parse(text)):
        if node.find("failure") is not None or node.find("error") is not None:
            outcome = "failed"
        elif node.find("skipped") is not None:
            outcome = "skipped"
        else:
            outcome = "passed"
        owner = suite.partition("::")[0] if language == "rust" else package
        name = node.get("name") or ""
        seconds = float(node.get("time") or 0)
        found.append(Case(category, language, owner, suite, name, outcome, seconds))
    return found


def merge(texts: Sequence[str]) -> str:
    """One JUnit document holding every suite of the given reports."""
    merged = ElementTree.Element("testsuites")
    for text in texts:
        root = _parse(text)
        if root.tag == "testsuite":
            merged.append(root)
            continue
        merged.extend(child for child in root if child.tag == "testsuite")
        orphans = [child for child in root if child.tag == "testcase"]
        if orphans:
            wrapper = ElementTree.SubElement(merged, "testsuite", name=root.get("name", ""))
            wrapper.extend(orphans)
    return ElementTree.tostring(merged, encoding="unicode")


def load(path: Path) -> dict:
    if not path.is_file():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def write(path: Path, text: str) -> None:
    """Replace a file atomically so a concurrent reader never sees half of it."""
    path.parent.mkdir(parents=True, exist_ok=True)
    staging = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    staging.write_text(text, encoding="utf-8")
    staging.replace(path)


def dump(path: Path, value: object) -> None:
    write(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


@contextmanager
def locked(directory: Path) -> Generator[None]:
    """Serialize read-modify-write of the shared evidence files across lanes."""
    path = directory / LOCK
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+b") as handle:
        if sys.platform == "win32":
            import msvcrt

            handle.seek(0)
            msvcrt.locking(handle.fileno(), msvcrt.LK_LOCK, 1)
        else:
            import fcntl

            fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        try:
            yield
        finally:
            if sys.platform == "win32":
                handle.seek(0)
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


# === Private Functions === #


def _parse(text: str) -> ElementTree.Element:
    try:
        return ElementTree.fromstring(text)
    except ElementTree.ParseError as error:
        raise policy.TestError(f"invalid JUnit report: {error}") from error


def _testcases(root: ElementTree.Element) -> Iterator[tuple[str, ElementTree.Element]]:
    """Every case with its suite: the classname, else the enclosing suite name."""
    if root.tag == "testsuites":
        for node in root.findall("testcase"):
            yield node.get("classname") or root.get("name") or "", node
    for suite in root.iter("testsuite"):
        for node in suite.findall("testcase"):
            yield node.get("classname") or suite.get("name") or "", node


# === Entry Point === #
