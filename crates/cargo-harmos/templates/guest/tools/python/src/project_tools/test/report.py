# Rendered by stkr project update from templates/python/test/report.py; sha256:b11e86fa4185b60cae9ce73d806fccdca84ece81b8456f49bdc8bd354c917f96
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Evidence Files: Lane CSVs, Merged JUnit, Manifest, Durations, Index And Benchmarks.

Everything lives under `.stkr/tests/` and `.stkr/benchmarks/`. Shared files are
rewritten under one lock with an atomic replace, because the language lanes of a layer run
concurrently.
"""

# === Imports === #

# Standard Library Imports
import csv
import io
import json
import os
import sys
from collections import defaultdict
from collections.abc import Generator, Iterator, Sequence
from contextlib import contextmanager
from dataclasses import astuple, dataclass, fields
from pathlib import Path
from xml.etree import ElementTree

# Third Party Imports

# Local Imports
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

TESTS = Path(".stkr/tests")
BENCHMARKS = Path(".stkr/benchmarks")
MANIFEST = TESTS / "manifest.json"
DURATIONS = TESTS / "durations.json"
INDEX = TESTS / "index.md"
JUNIT = TESTS / "junit.xml"
BENCH_CSV = BENCHMARKS / "bench.csv"
BENCH_MARKDOWN = BENCHMARKS / "bench.md"
LOCK = TESTS / ".lock"
SCHEMA = 1
CASE_HEADER = [
    "category",
    "language",
    "package",
    "file",
    "story",
    "outcome",
    "duration_ms",
    "deadline_ms",
]

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


def lane_csv(found: Sequence[Case], deadlines: dict[tuple[str, str], int]) -> str:
    buffer = io.StringIO()
    writer = csv.writer(buffer, lineterminator="\n")
    writer.writerow(CASE_HEADER)
    for case in found:
        deadline = deadlines.get((case.suite, case.name), 0)
        writer.writerow(
            [
                case.category,
                case.language,
                case.package,
                case.file,
                case.story,
                case.outcome,
                round(case.seconds * 1000),
                deadline * 1000,
            ]
        )
    return buffer.getvalue()


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


def index(directory: Path, rules: policy.Policy) -> str:
    """The one-page overview: layers against caps, lanes, benchmarks, then every story."""
    manifest = load(directory / MANIFEST)
    durations = load(directory / DURATIONS)
    lanes = manifest.get("lanes", {})
    rows = [row for key in sorted(lanes) if lanes[key].get("kind") == "test" for row in read_csv(directory / TESTS / lanes[key]["csv"])]

    # Header And Layer Caps
    lines = [
        "# Test Index",
        "",
        f"Tree `{manifest.get('tree', '-')}`, revision `{manifest.get('revision') or 'uncommitted'}`, profile `{manifest.get('profile', '-')}`.",
        "",
        "## Layers",
        "",
        "| Layer | Stories | Cap | Finding |",
        "|---|---|---|---|",
    ]
    counts = defaultdict(int)
    for lane in lanes.values():
        if lane.get("kind") == "test":
            counts[lane["category"]] += lane.get("tests", 0)
    for category in policy.CATEGORIES:
        count, cap = counts[category], rules.limits[category]
        finding = f"over cap by {count - cap}; curate" if count > cap else ""
        lines.append(f"| {category.title()} | {count} | {cap} | {finding} |")

    # Lanes
    lines += [
        "",
        "## Lanes",
        "",
        "| Lane | Outcome | Stories | Failed | Seconds | Focused |",
        "|---|---|---|---|---|---|",
    ]
    for key in sorted(lanes):
        lane = lanes[key]
        focused = "yes" if lane.get("focused") else ""
        lines.append(
            f"| {key} | {lane.get('outcome')} | {lane.get('tests', 0)} | {lane.get('failed', 0)} | {lane.get('seconds', 0):.1f} | {focused} |"
        )

    # Benchmarks
    benches = parse_bench(directory / BENCH_CSV)
    if benches:
        lines += ["", "## Benchmarks", "", *bench_table(benches, changed_only=True)]

    # Stories Grouped By Package, File And Story
    grouped: dict[str, dict[str, list[dict[str, str]]]] = defaultdict(lambda: defaultdict(list))
    for row in rows:
        grouped[row["package"]][row["file"]].append(row)
    drift = _drift(durations)
    for package in sorted(grouped):
        lines += ["", f"## {package}"]
        for file in sorted(grouped[package]):
            lines += [
                "",
                f"### {file}",
                "",
                "| Story | Layer | Language | Outcome | Ms | Deadline | Drift |",
                "|---|---|---|---|---|---|---|",
            ]
            for row in sorted(grouped[package][file], key=lambda item: item["story"]):
                change = drift.get((row["category"], row["language"], package, file, row["story"]), "")
                deadline = int(row["deadline_ms"]) // 1000
                lines.append(
                    f"| {_cell(row['story'])} | {row['category']} | {row['language']} | {row['outcome']} | {row['duration_ms']} | {deadline or ''} | {change} |"
                )
    return "\n".join(lines) + "\n"


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


def _drift(durations: dict) -> dict[tuple[str, str, str, str, str], str]:
    """Percent change of each story's last duration against the one before it."""
    shown = {}
    for key, suites in durations.items():
        category, _, language = key.partition("-")
        for suite, stories in suites.items():
            for name, record in stories.items():
                previous = record.get("previous")
                if not previous:
                    continue
                case = Case(category, language, "", suite, name, "passed", 0.0)
                owner = suite.partition("::")[0] if language == "rust" else record.get("package", "")
                change = (record["seconds"] - previous) / previous * 100
                shown[(category, language, owner, case.file, case.story)] = f"{change:+.0f}%"
    return shown


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
