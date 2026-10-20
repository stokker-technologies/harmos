# Rendered by stkr project update from templates/python/test/report.py; sha256:97ed42f1e9c0de92a2f103350f89af6ab9d48858745c3db1ca7c1be01ba6067a
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Read Native JUnit Reports And Render Benchmark Tables."""

# === Imports === #

# Standard Library Imports
import csv
import io
import json
import os
import sys
from collections.abc import Generator, Iterator, Sequence
from contextlib import contextmanager
from dataclasses import astuple, dataclass, fields
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
BENCHMARKS = Path(".stkr/benchmarks")
BENCH_CSV = BENCHMARKS / "bench.csv"
BENCH_MARKDOWN = BENCHMARKS / "bench.md"
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


@dataclass(frozen=True)
class Bench:
    category: str
    language: str
    domain: str
    name: str
    samples: int
    median_ns: float
    mean_ns: float
    p95_ns: float | None
    stddev_ns: float | None
    peak_bytes: int | None
    baseline_ns: float | None

    @property
    def change_pct(self) -> float | None:
        if not self.baseline_ns:
            return None
        return (self.median_ns - self.baseline_ns) / self.baseline_ns * 100


# === Public Functions === #


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


def read_csv(path: Path) -> list[dict[str, str]]:
    if not path.is_file():
        return []
    return list(csv.DictReader(io.StringIO(path.read_text(encoding="utf-8"))))


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


def bench_csv(rows: Sequence[Bench]) -> str:
    buffer = io.StringIO()
    writer = csv.writer(buffer, lineterminator="\n")
    writer.writerow([field.name for field in fields(Bench)])
    for row in rows:
        writer.writerow(["" if value is None else value for value in astuple(row)])
    return buffer.getvalue()


def parse_bench(path: Path) -> list[Bench]:
    rows = []
    for record in read_csv(path):
        rows.append(
            Bench(
                record["category"],
                record["language"],
                record["domain"],
                record["name"],
                int(record["samples"]),
                float(record["median_ns"]),
                float(record["mean_ns"]),
                _float(record["p95_ns"]),
                _float(record["stddev_ns"]),
                _int(record["peak_bytes"]),
                _float(record["baseline_ns"]),
            )
        )
    return rows


def bench_table(rows: Sequence[Bench], changed_only: bool = False) -> list[str]:
    """Benchmarks against the `dev` baseline; the index lists only measured changes."""
    lines = [
        "| Layer | Language | Domain | Benchmark | Median | Baseline | Change | Peak |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for row in rows:
        change = row.change_pct
        if changed_only and change is None:
            continue
        peak = f"{row.peak_bytes / 1_048_576:.1f} MiB" if row.peak_bytes else ""
        shown = f"{change:+.1f}%" if change is not None else ""
        baseline = _time(row.baseline_ns) if row.baseline_ns else ""
        lines.append(
            f"| {row.category} | {row.language} | {row.domain} | {_cell(row.name)} | {_time(row.median_ns)} | {baseline} | {shown} | {peak} |"
        )
    return lines


def bench_markdown(rows: Sequence[Bench]) -> str:
    return (
        "\n".join(
            [
                "# Benchmarks",
                "",
                "Compared with the `dev` baseline of the last dev-landed revision.",
                "",
                *bench_table(rows),
            ]
        )
        + "\n"
    )


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


def _float(text: str) -> float | None:
    return float(text) if text else None


def _int(text: str) -> int | None:
    return int(text) if text else None


def _time(nanoseconds: float) -> str:
    for unit, scale in (("s", 1e9), ("ms", 1e6), ("µs", 1e3)):
        if nanoseconds >= scale:
            return f"{nanoseconds / scale:.2f} {unit}"
    return f"{nanoseconds:.0f} ns"


def _cell(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


# === Entry Point === #
