# Rendered by stkr project update from templates/python/test/run.py; sha256:dd3d3af200d3eb36dac8709365004190f16709bd4be00609b9ed1f75982357ce
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Run Archived Test Lanes And Keep Native JUnit Reports Under Target."""

# === Imports === #

# Standard Library Imports
import argparse
import json
import math
import os
import platform
import shutil
import statistics
import subprocess
import sys
import time
from collections.abc import Generator, Sequence
from contextlib import contextmanager
from pathlib import Path

# Third Party Imports
# Local Imports
from .. import temporary
from ..build import paths
from . import archive, nextest, policy, pytest, report, vitest

# Type Checking Imports

# === Public Exports === #

# === Constants === #

COVERAGE_VARIABLE = "STKR_COVERAGE"
CRITERION = report.BENCHMARKS / "criterion"
BASELINE = "dev"
SCRIPTS = ("typescript", "javascript")

# === Types === #

# === Classes === #

# === Public Functions === #


def test(
    directory: Path,
    category: str,
    language: str,
    project: str,
    profile: str,
    extra: Sequence[str],
) -> int:
    """Run One Lane And Retain Its Native Report; Nonzero When Any Story Failed."""
    rules = policy.load(directory)
    key = f"{category}-{language}"
    _preflight(directory, category, language, profile)
    if language == "python":
        archive.install_wheels(directory, profile)
    recorded = report.load(report.location(directory, profile, key) / "durations.json")
    with _scratch(directory, key, profile) as scratch:
        deadline = rules.deadline(category)

        # Native Command
        environment = None
        cwd = directory / project
        if language == "rust":
            path = archive.require(directory, profile)
            binaries = archive.binaries(directory, profile)
            names = sorted(set(binaries.values()))
            expression, rest = nextest.selection(category, extra, names)
            measured = nextest.overrides(rules, category, recorded, coverage=False, binaries=binaries)
            configuration = scratch / "nextest.toml"
            store = str(scratch / "store")
            configuration.write_text(nextest.config(store, deadline, measured), encoding="utf-8")
            junit = scratch / "store" / nextest.PROFILE / "junit.xml"
            command = nextest.run(path, directory, configuration, scratch / "extract", expression, rest)
            cwd = directory
        elif language == "python":
            junit = scratch / "junit.xml"
            command = pytest.run(cwd, f"tests/{category}", deadline, junit, extra)
            environment = pytest.environment(directory)
        else:
            junit = scratch / "junit.xml"
            command = vitest.run(extra, junit, deadline)

        lane = _lane(
            directory,
            rules,
            key,
            category,
            language,
            project,
            profile,
            command,
            cwd,
            environment,
            junit,
            recorded,
        )
        lane["arguments"] = list(extra)
        return 0 if lane["outcome"] == "passed" else 1


def coverage(
    directory: Path,
    language: str,
    category: str | None,
    project: str,
    profile: str,
    extra: Sequence[str],
) -> int:
    """Instrumented lane over one layer, or every layer when no category is given."""
    rules = policy.load(directory)
    key = f"coverage-{category}-{language}" if category else f"coverage-{language}"
    _preflight(directory, category, language, profile)
    with _scratch(directory, key, profile) as scratch:
        categories = [category] if category else list(policy.CATEGORIES)
        deadline = max(rules.deadline(name, coverage=True) for name in categories)
        cobertura = scratch / "coverage.xml"
        environment = dict(os.environ, **{COVERAGE_VARIABLE: "1"})

        # Instrumented Run
        if language == "rust":
            binaries = archive.binaries(directory, profile)
            names = sorted(set(binaries.values()))
            expression, rest = nextest.selection(category, extra, names)
            measured = nextest.overrides(rules, category, {}, coverage=True, binaries=binaries)
            configuration = scratch / "nextest.toml"
            store = str(scratch / "store")
            configuration.write_text(nextest.config(store, deadline, measured), encoding="utf-8")
            junit = scratch / "store" / nextest.PROFILE / "junit.xml"
            command = nextest.coverage(profile, configuration, expression, rest)
            cwd = directory
        elif language == "python":
            cwd = directory / project
            junit = scratch / "junit.xml"
            suite = f"tests/{category}" if category else "tests"
            command = pytest.run(cwd, suite, deadline, junit, [*extra, *pytest.coverage(cwd, cobertura)])
            environment = {**pytest.environment(directory), COVERAGE_VARIABLE: "1"}
        else:
            raise policy.TestError("coverage is measured for rust and python")

        lane = _lane(
            directory,
            rules,
            key,
            category or "coverage",
            language,
            project,
            profile,
            command,
            cwd,
            environment,
            junit,
            {},
        )
        if language == "rust" and _execute(nextest.report(profile, cobertura), directory, environment) != 0:
            lane["outcome"] = "failed"
        if not cobertura.is_file():
            raise policy.TestError(f"{key}: no coverage report was written")
        lane["kind"] = "coverage"
        lane["arguments"] = list(extra)
        lane["coverage"] = (report.location(directory, profile, key) / "coverage.xml").relative_to(directory).as_posix()
        report.write(
            report.location(directory, profile, key) / "coverage.xml",
            cobertura.read_text(encoding="utf-8"),
        )
        return 0 if lane["outcome"] == "passed" else 1


def bench(
    directory: Path,
    category: str,
    language: str,
    project: str,
    refresh: bool,
    extra: Sequence[str],
    archive_arguments: Sequence[str] = (),
) -> int:
    """Measure one benchmark layer and compare it with the `dev` baseline."""
    rules = policy.load(directory)
    measure = category in ("integration", "system")
    _preflight(directory, category, language, "bench")
    identity = {
        "platform": [
            sys.platform,
            platform.machine(),
            platform.platform(),
            platform.processor(),
        ],
        "runner": os.environ.get("RUNNER_NAME", os.environ.get("CI_RUNNER_ID", platform.node())),
        "profile": "bench",
        "metric": "median-nanoseconds",
        "arguments": list(extra),
        "cargo_arguments": list(archive_arguments),
    }
    if language == "rust":
        identity["rustc"] = archive._toolchain(directory)
        rows, status = _bench_rust(directory, category, refresh, measure, extra, archive_arguments, identity)
    elif language == "python":
        identity["python"] = {
            "version": sys.version,
            "executable": archive._digest(Path(sys.executable)),
        }
        rows, status = _bench_python(directory, category, project, refresh, measure, extra, identity)
    else:
        raise policy.TestError("benchmarks run for rust and python")
    if not rows:
        raise policy.TestError(f"no {category} {language} benchmark produced results")

    kept = [row for row in report.parse_bench(directory / report.BENCH_CSV) if (row.category, row.language) != (category, language)]
    merged = sorted(
        [*kept, *rows],
        key=lambda row: (row.category, row.language, row.domain, row.name),
    )
    report.write(directory / report.BENCH_CSV, report.bench_csv(merged))
    report.write(directory / report.BENCH_MARKDOWN, report.bench_markdown(merged))
    for row in rows:
        if row.change_pct is not None and math.isfinite(row.change_pct) and row.change_pct > rules.bench_regression_percent:
            print(f"benchmark regression: {row.name}: {row.change_pct:.1f}%")
    print(f"bench {category}-{language}: {len(rows)} results; see {report.BENCH_MARKDOWN}")
    return 0 if status == 0 else 1


def main(arguments: Sequence[str] | None = None) -> int:
    values = list(sys.argv[1:] if arguments is None else arguments)
    extra: list[str] = []
    if "--" in values:
        split = values.index("--")
        values, extra = values[:split], values[split + 1 :]

    parser = argparse.ArgumentParser(description="Test Lanes And Evidence")
    commands = parser.add_subparsers(dest="command", required=True)
    lane = commands.add_parser("test", help="run one test lane")
    lane.add_argument("--category", choices=policy.CATEGORIES, required=True)
    lane.add_argument("--language", choices=policy.LANGUAGES, required=True)
    lane.add_argument("--coverage", action="store_true")
    bench_lane = commands.add_parser("bench", help="run one benchmark layer")
    bench_lane.add_argument("--category", choices=policy.BENCH_CATEGORIES, required=True)
    bench_lane.add_argument("--language", choices=("rust", "python"), required=True)
    bench_lane.add_argument("--refresh-baseline", action="store_true")
    bench_lane.add_argument("--archive-argument", action="append", default=[])
    measured = commands.add_parser("coverage", help="instrumented run of every layer")
    measured.add_argument("--language", choices=("rust", "python"), required=True)
    for command in (lane, bench_lane, measured):
        command.add_argument("--project", default=".")
    for command in (lane, measured):
        command.add_argument("--profile", choices=policy.PROFILES, default="dev")
    options = parser.parse_args(values)

    try:
        directory = policy.root()
        if options.command == "test" and options.coverage:
            return coverage(
                directory,
                options.language,
                options.category,
                options.project,
                options.profile,
                extra,
            )
        if options.command == "test":
            return test(
                directory,
                options.category,
                options.language,
                options.project,
                options.profile,
                extra,
            )
        if options.command == "coverage":
            return coverage(
                directory,
                options.language,
                None,
                options.project,
                options.profile,
                extra,
            )
        if options.command == "bench":
            return bench(
                directory,
                options.category,
                options.language,
                options.project,
                options.refresh_baseline,
                extra,
                options.archive_argument,
            )
        return 0
    except (policy.TestError, OSError, ValueError, KeyError) as error:
        print(f"test: {error}", file=sys.stderr)
        return 1


# === Private Functions === #


def _preflight(
    directory: Path,
    category: str | None,
    language: str,
    profile: str,
) -> None:
    """Refuse a stale fixture stamp before any Rust test runs."""
    if language == "rust" and (category != "unit" or profile == "bench"):
        output = paths.output(directory, "build", profile).relative_to(directory)
        archive.fixtures(directory, profile, roots=[(output / part).as_posix() for part in ("tests", "fixtures")])


@contextmanager
def _scratch(directory: Path, key: str, profile: str) -> Generator[Path]:
    if not key.startswith("bench-"):
        output = report.location(directory, profile, key)
        if output.is_symlink() or not output.resolve().is_relative_to(directory.resolve()):
            raise policy.TestError("lane output escapes the repository")
        shutil.rmtree(output, ignore_errors=True)
    with temporary.directory() as scratch:
        yield scratch


def _lane(
    directory: Path,
    rules: policy.Policy,
    key: str,
    category: str,
    language: str,
    project: str,
    profile: str,
    command: Sequence[str],
    cwd: Path,
    environment: dict[str, str] | None,
    junit: Path,
    recorded: dict[str, dict],
) -> dict:
    """Execute a lane command and turn its JUnit report into a lane record."""
    started = time.time()
    status = _execute(command, cwd, environment)
    seconds = time.time() - started
    if not junit.is_file():
        raise policy.TestError(f"{key}: the runner exited {status} without a JUnit report")

    # Stories, Deadlines And Liveness Overruns
    text = junit.read_text(encoding="utf-8")
    package = Path(project).name if project != "." else directory.name
    found = report.cases(text, category, language, package)
    deadlines = {(case.suite, case.name): _deadline(rules, language, case, recorded) for case in found}
    overruns = [
        f"{case.file}::{case.story}"
        for case in found
        if language != "rust" and case.outcome == "passed" and case.seconds > deadlines[(case.suite, case.name)]
    ]
    failed = sum(case.outcome == "failed" for case in found) + len(overruns)
    passed = sum(case.outcome == "passed" for case in found) - len(overruns)
    for story in overruns:
        print(f"{key}: {story} exceeded its liveness deadline", file=sys.stderr)

    destination = report.location(directory, profile, key)
    report.write(destination / "junit.xml", text)
    durations = {}
    for case in found:
        if case.outcome == "passed":
            durations.setdefault(case.suite, {})[case.name] = {"seconds": case.seconds}
    report.dump(destination / "durations.json", durations)
    print(f"{key}: {passed} passed, {failed} failed, {len(found) - passed - failed} skipped")
    ran = passed > 0 or failed > 0
    return {
        "category": category,
        "language": language,
        "kind": "test",
        "outcome": "passed" if status == 0 and ran and not failed else "failed",
        "project": project,
        "profile": profile,
        "tests": len(found),
        "passed": passed,
        "failed": failed,
        "skipped": sum(case.outcome == "skipped" for case in found),
        "overruns": overruns,
        "seconds": round(seconds, 3),
        "started_at": int(started),
        "junit": (destination / "junit.xml").relative_to(directory).as_posix(),
        "coverage": None,
        "durations": [
            (case.suite, case.name, case.package, case.seconds)
            for case in found
            if case.outcome == "passed" and f"{case.file}::{case.story}" not in overruns
        ],
    }


def _deadline(rules: policy.Policy, language: str, case: report.Case, recorded: dict) -> int:
    if language != "rust":
        entry = rules.matched(case.category, language, f"{case.file}::{case.story}")
        if entry is not None:
            return entry.timeout_seconds
    if case.category not in policy.CATEGORIES:
        return 0
    seconds = recorded.get(case.suite, {}).get(case.name, {}).get("seconds")
    return rules.story(case.category, seconds)


def _execute(command: Sequence[str], cwd: Path, environment: dict[str, str] | None) -> int:
    try:
        return subprocess.run(list(command), cwd=cwd, env=environment, check=False).returncode
    except OSError as error:
        raise policy.TestError(f"{command[0]} is unavailable: {error}") from error


def _measured(command: Sequence[str], cwd: Path, environment: dict[str, str]) -> tuple[int, int | None]:
    """Exit status and peak resident bytes of one process and the children it waited for."""
    try:
        process = subprocess.Popen(list(command), cwd=cwd, env=environment)
    except OSError as error:
        raise policy.TestError(f"{command[0]} is unavailable: {error}") from error
    if not hasattr(os, "wait4"):
        return process.wait(), None
    _, status, usage = os.wait4(process.pid, 0)
    process.returncode = os.waitstatus_to_exitcode(status)
    scale = 1 if sys.platform == "darwin" else 1024
    return process.returncode, usage.ru_maxrss * scale


def _bench_rust(
    directory: Path,
    category: str,
    refresh: bool,
    measure: bool,
    extra: Sequence[str],
    archive_arguments: Sequence[str],
    identity: dict,
) -> tuple[list[report.Bench], int]:
    messages = paths.output(directory, "build", "bench") / "bench/compiler.json"
    if not messages.is_file():
        raise policy.TestError("missing benchmark compiler metadata; run build:bench first")
    archive.fixtures(directory, "bench", [messages.relative_to(directory).as_posix()])
    targets = []
    for line in messages.read_text(encoding="utf-8").splitlines():
        message = json.loads(line) if line.startswith("{") else {}
        target = message.get("target", {})
        name = target.get("name", "")
        artifact = message.get("reason") == "compiler-artifact" and "bench" in target.get("kind", []) and message.get("executable")
        if artifact and (name == category or name.startswith(category + "-")):
            targets.append(
                (
                    name,
                    str(directory / message["executable"]),
                    (directory / message["manifest_path"]).parent,
                )
            )

    home = directory / CRITERION
    baselines = _criterion_baselines(home, identity)
    saving = refresh or not baselines
    flags = ["--save-baseline", BASELINE] if saving else ["--baseline-lenient", BASELINE]
    environment = dict(os.environ, CRITERION_HOME=str(home))
    rows: list[report.Bench] = []
    status = 0
    for name, executable, package in targets:
        since = time.time()
        code, peak = _measured([executable, "--bench", *flags, *extra], package, environment)
        status = status or code
        domain = name.removeprefix(category).removeprefix("-") or name
        rows += _criterion_rows(home, category, domain, since, peak if measure else None, baselines)
        if saving and code == 0:
            for path in home.rglob(f"{BASELINE}/estimates.json"):
                if path.stat().st_mtime >= since:
                    report.write(
                        path.with_name("inputs.json"),
                        json.dumps(
                            {"identity": identity, "sha256": archive._digest(path)},
                            indent=2,
                        )
                        + "\n",
                    )
    return rows, status


def _criterion_baselines(home: Path, identity: dict) -> dict[Path, float]:
    """Median of every benchmark's `dev` baseline before this run replaces anything."""
    medians = {}
    for path in home.rglob(f"{BASELINE}/estimates.json") if home.is_dir() else []:
        try:
            recorded = report.load(path.with_name("inputs.json"))
            if not isinstance(recorded, dict) or recorded.get("identity") != identity or recorded.get("sha256") != archive._digest(path):
                continue
            median = json.loads(path.read_text(encoding="utf-8"))["median"]["point_estimate"]
        except (OSError, ValueError, KeyError, TypeError):
            continue
        if not isinstance(median, bool) and isinstance(median, (int, float)) and math.isfinite(median) and median > 0:
            medians[path.parent.parent] = median
    return medians


def _criterion_rows(
    home: Path,
    category: str,
    domain: str,
    since: float,
    peak: int | None,
    baselines: dict[Path, float],
) -> list[report.Bench]:
    rows = []
    for path in sorted(home.rglob("new/benchmark.json")) if home.is_dir() else []:
        if path.stat().st_mtime < since:
            continue
        identity = json.loads(path.read_text(encoding="utf-8"))
        estimates = json.loads((path.parent / "estimates.json").read_text(encoding="utf-8"))
        sample = json.loads((path.parent / "sample.json").read_text(encoding="utf-8"))
        per_iteration = [time / iterations for time, iterations in zip(sample["times"], sample["iters"], strict=True)]
        rows.append(
            report.Bench(
                category,
                "rust",
                domain,
                identity.get("full_id", path.parent.parent.name),
                len(per_iteration),
                estimates["median"]["point_estimate"],
                estimates["mean"]["point_estimate"],
                _percentile(per_iteration, 0.95),
                estimates.get("std_dev", {}).get("point_estimate"),
                peak,
                baselines.get(path.parent.parent),
            )
        )
    return rows


def _bench_python(
    directory: Path,
    category: str,
    project: str,
    refresh: bool,
    measure: bool,
    extra: Sequence[str],
    identity: dict,
) -> tuple[list[report.Bench], int]:
    with _scratch(directory, f"bench-{category}-python", "bench") as scratch:
        output = scratch / "benchmarks.json"
        cwd = directory / project
        status, peak = _measured(
            pytest.bench(cwd, category, output, extra),
            cwd,
            pytest.environment(directory),
        )
        if not output.is_file():
            raise policy.TestError("pytest-benchmark wrote no results")

        # The Baseline Is This Runner's Own Record Of Medians
        stored = directory / report.BENCHMARKS / f"python-{category}-{BASELINE}.json"
        try:
            baseline = report.load(stored)
        except (OSError, ValueError):
            baseline = {}
        baselines = baseline.get("medians", {}) if isinstance(baseline, dict) and baseline.get("identity") == identity else {}
        baselines = baselines if isinstance(baselines, dict) else {}
        baselines = {
            name: median
            for name, median in baselines.items()
            if not isinstance(median, bool) and isinstance(median, (int, float)) and math.isfinite(median) and median > 0
        }
        rows = []
        for entry in json.loads(output.read_text(encoding="utf-8"))["benchmarks"]:
            stats = entry["stats"]
            data = [value * 1e9 for value in stats.get("data", [])]
            name = entry.get("fullname", entry["name"])
            rows.append(
                report.Bench(
                    category,
                    "python",
                    entry.get("group") or Path(name.partition("::")[0]).stem,
                    name,
                    stats.get("rounds", len(data)),
                    stats["median"] * 1e9,
                    stats["mean"] * 1e9,
                    _percentile(data, 0.95),
                    stats.get("stddev", 0) * 1e9,
                    peak if measure else None,
                    baselines.get(name),
                )
            )
        if refresh or not baselines:
            report.dump(
                stored,
                {
                    "schema": 1,
                    "identity": identity,
                    "medians": {row.name: row.median_ns for row in rows},
                },
            )
        return rows, status


def _percentile(values: Sequence[float], quantile: float) -> float | None:
    if len(values) < 2:
        return values[0] if values else None
    return statistics.quantiles(values, n=100, method="inclusive")[round(quantile * 100) - 1]


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
