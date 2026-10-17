# Rendered by stkr project update from templates/python/test/run.py; sha256:ccdb021805fa0887790d42f83416388285c8e56e5f6fcb940efde05f248781a6
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Run Test Lanes, Coverage And Benchmarks And Publish Tree-Bound Evidence.

    python -m <package>.test.run test --category <unit|integration|acceptance|system>
        --language <rust|python|typescript|javascript> [--project DIR]
        [--profile dev|release] [--coverage] [-- ARGS...]
    python -m <package>.test.run bench --category <unit|integration|system>
        --language <rust|python> [--project DIR] [--refresh-baseline] [-- ARGS...]
    python -m <package>.test.run coverage --language <rust|python> [--project DIR]
        [--profile dev|release] [-- ARGS...]
    python -m <package>.test.run verify [--category ...]
    python -m <package>.test.run index
    python -m <package>.test.run collect <directory> ...

Rust lanes run from the nextest archive of their profile (`python -m <package>.test.archive`)
and never compile; ARGS narrow the selection (`-p crate`, `-E filterset`) and mark the lane
focused. Python lanes run pytest over `<project>/tests/<category>/` in the root `.venv/`;
ARGS are appended to pytest. Script lanes run ARGS as the command (see `vitest`). Before any
test runs, a lane checks its archive, the fixture stamp and, for acceptance and system, the
declared host artifacts, all against the current tree hash.

Acceptance and system stories get a liveness deadline of about three times their recorded
duration within the lane maximum; durations persist in `.stkr/tests/durations.json`.
Coverage is instrumented, covers every layer of its language at five times the deadlines and
is the one Rust job that compiles.

Benchmarks: a Rust bench target belongs to a layer by name, `<layer>` or `<layer>-<domain>`
(declare `[[bench]] name = "integration-storage", path = "benches/storage/main.rs",
harness = false`); Python benchmarks live in `<project>/benchmarks/<layer>/`. Criterion keeps
its baseline named `dev` under `.stkr/benchmarks/criterion/`; runs compare against it,
the first run saves it and `--refresh-baseline` replaces it after a dev landing. Integration
and system rows record the peak resident memory of the benchmark process.

`verify` exits nonzero unless every `test:<category>:<language>` task of `mise.toml` has a
passed, unfocused lane for the current tree hash, and lists what is missing.
"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import time
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports

# Local Imports
from . import archive, nextest, policy, pytest, report, vitest

# Type Checking Imports

# === Public Exports === #

# === Constants === #

MANIFEST = report.MANIFEST
COVERAGE_VARIABLE = "STKR_COVERAGE"
SCRATCH = Path("target/tests")
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
    """Run one lane and publish its evidence; nonzero when any story failed."""
    rules = policy.load(directory)
    current = archive.tree(directory)
    key = f"{category}-{language}"
    _preflight(directory, rules, category, profile, current)
    recorded = report.load(directory / report.DURATIONS).get(key, {})
    scratch = _scratch(directory, key)
    deadline = rules.deadline(category)

    # Native Command
    environment = None
    cwd = directory / project
    if language == "rust":
        path = archive.require(directory, profile, current)
        names = archive.binaries(directory, profile)
        expression, rest = nextest.selection(category, extra, names)
        measured = nextest.overrides(rules, category, recorded, coverage=False, binaries=names)
        configuration = scratch / "nextest.toml"
        store = (scratch / "store").relative_to(directory).as_posix()
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
    lane["focused"] = bool(extra) and language not in SCRIPTS
    lane["arguments"] = list(extra)
    return _publish(directory, rules, current, profile, key, lane, recorded)


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
    current = archive.tree(directory)
    key = f"coverage-{category}-{language}" if category else f"coverage-{language}"
    _preflight(directory, rules, category, profile, current)
    scratch = _scratch(directory, key)
    categories = [category] if category else list(policy.CATEGORIES)
    deadline = max(rules.deadline(name, coverage=True) for name in categories)
    cobertura = scratch / "coverage.xml"
    environment = dict(os.environ, **{COVERAGE_VARIABLE: "1"})

    # Instrumented Run
    if language == "rust":
        names = archive.binaries(directory, profile)
        expression, rest = nextest.selection(category, extra, names)
        measured = nextest.overrides(rules, category, {}, coverage=True, binaries=names)
        configuration = scratch / "nextest.toml"
        store = (scratch / "store").relative_to(directory).as_posix()
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
    lane["focused"] = bool(extra) or category is not None
    lane["arguments"] = list(extra)
    lane["coverage"] = f"{key}.cobertura.xml"
    report.write(
        directory / report.TESTS / lane["coverage"],
        cobertura.read_text(encoding="utf-8"),
    )
    return _publish(directory, rules, current, profile, key, lane, {})


def bench(
    directory: Path,
    category: str,
    language: str,
    project: str,
    refresh: bool,
    extra: Sequence[str],
) -> int:
    """Measure one benchmark layer and compare it with the `dev` baseline."""
    rules = policy.load(directory)
    measure = category in ("integration", "system")
    if language == "rust":
        rows, status = _bench_rust(directory, category, refresh, measure, extra)
    elif language == "python":
        rows, status = _bench_python(directory, category, project, refresh, measure, extra)
    else:
        raise policy.TestError("benchmarks run for rust and python")
    if not rows:
        raise policy.TestError(f"no {category} {language} benchmark produced results")

    with report.locked(directory):
        kept = [row for row in report.parse_bench(directory / report.BENCH_CSV) if (row.category, row.language) != (category, language)]
        merged = sorted(
            [*kept, *rows],
            key=lambda row: (row.category, row.language, row.domain, row.name),
        )
        report.write(directory / report.BENCH_CSV, report.bench_csv(merged))
        report.write(directory / report.BENCH_MARKDOWN, report.bench_markdown(merged))
        report.write(directory / report.INDEX, report.index(directory, rules))
    print(f"bench {category}-{language}: {len(rows)} results; see {report.BENCH_MARKDOWN}")
    return 0 if status == 0 else 1


def missing(directory: Path, keys: Sequence[str], revision: str | None = None) -> list[str]:
    """The lanes without passed, unfocused evidence for the current tree (and revision)."""
    manifest = report.load(directory / MANIFEST)
    if manifest.get("tree") != archive.tree(directory):
        return [f"{key} (no evidence for the current tree)" for key in keys]
    if revision is not None and manifest.get("revision") != revision:
        return [f"{key} (evidence is not bound to {revision})" for key in keys]

    absent = []
    for key in keys:
        lane = manifest.get("lanes", {}).get(key)
        if lane is None:
            absent.append(f"{key} (not run)")
        elif lane.get("outcome") != "passed":
            absent.append(f"{key} ({lane.get('outcome')})")
        elif lane.get("focused") or lane.get("kind") != "test":
            absent.append(f"{key} (focused run)")
    return absent


def verify(directory: Path, categories: Sequence[str]) -> int:
    expected = [f"{category}-{language}" for category, language in policy.lanes(directory) if not categories or category in categories]
    if not expected:
        raise policy.TestError("mise.toml declares no matching test:<category>:<language> tasks")
    absent = missing(directory, expected)
    for line in absent:
        print(f"missing: {line}")
    if not absent:
        print(f"verified {len(expected)} lanes for the current tree")
    return 1 if absent else 0


def collect(directory: Path, sources: Sequence[Path]) -> int:
    """Aggregate lane evidence downloaded from CI jobs into one manifest for this tree."""
    current = archive.tree(directory)
    documents = []
    for source in sources:
        manifest = report.load(source / "manifest.json")
        if manifest.get("schema") != report.SCHEMA or manifest.get("tree") != current:
            raise policy.TestError(f"{source}: evidence is missing or from another tree")
        documents.append((source, manifest))
    profiles = {manifest.get("profile") for _, manifest in documents}
    if len(profiles) != 1:
        raise policy.TestError("collected evidence mixes profiles")

    with report.locked(directory):
        target = directory / report.TESTS
        lanes: dict[str, dict] = {}
        durations = report.load(directory / report.DURATIONS)
        for source, manifest in documents:
            for key, lane in manifest.get("lanes", {}).items():
                if key in lanes:
                    raise policy.TestError(f"lane {key} was collected twice")
                for name in (lane.get("csv"), lane.get("junit"), lane.get("coverage")):
                    if name:
                        shutil.copyfile(source / name, target / name)
                lanes[key] = lane
            durations.update(report.load(source / "durations.json"))
        manifest = {
            "schema": report.SCHEMA,
            "tree": current,
            "revision": archive.published(directory, current),
            "profile": profiles.pop(),
            "lanes": lanes,
        }
        _store(directory, manifest, durations, policy.load(directory))
    print(f"collected {len(lanes)} lanes")
    return 0


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
    measured = commands.add_parser("coverage", help="instrumented run of every layer")
    measured.add_argument("--language", choices=("rust", "python"), required=True)
    for command in (lane, bench_lane, measured):
        command.add_argument("--project", default=".")
    for command in (lane, measured):
        command.add_argument("--profile", choices=policy.PROFILES, default="dev")
    verified = commands.add_parser("verify", help="check the evidence set of this tree")
    verified.add_argument("--category", choices=policy.CATEGORIES, action="append", default=[])
    commands.add_parser("index", help="render the evidence index again")
    collected = commands.add_parser("collect", help="aggregate downloaded CI evidence")
    collected.add_argument("directories", type=Path, nargs="+")
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
            )
        if options.command == "verify":
            return verify(directory, options.category)
        if options.command == "collect":
            return collect(
                directory,
                [path if path.is_absolute() else directory / path for path in options.directories],
            )
        with report.locked(directory):
            report.write(
                directory / report.INDEX,
                report.index(directory, policy.load(directory)),
            )
        return 0
    except (policy.TestError, OSError, ValueError, KeyError) as error:
        print(f"test: {error}", file=sys.stderr)
        return 1


# === Private Functions === #


def _preflight(
    directory: Path,
    rules: policy.Policy,
    category: str | None,
    profile: str,
    current: str,
) -> None:
    """Refuse a stale fixture stamp or absent host artifacts before any test runs."""
    archive.fixtures(directory, current)
    if category in (None, "acceptance", "system"):
        hosts = [host.replace("{profile}", profile) for host in rules.hosts]
        absent = [host for host in hosts if not (directory / host).exists()]
        if absent:
            raise policy.TestError("missing host artifacts: " + ", ".join(absent))


def _scratch(directory: Path, key: str) -> Path:
    scratch = directory / SCRATCH / key
    shutil.rmtree(scratch, ignore_errors=True)
    scratch.mkdir(parents=True)
    return scratch


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

    report.write(directory / report.TESTS / f"{key}.junit.xml", text)
    report.write(directory / report.TESTS / f"{key}.csv", report.lane_csv(found, deadlines))
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
        "csv": f"{key}.csv",
        "junit": f"{key}.junit.xml",
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


def _publish(
    directory: Path,
    rules: policy.Policy,
    current: str,
    profile: str,
    key: str,
    lane: dict,
    recorded: dict,
) -> int:
    """Record the lane in the manifest of this tree and profile, then refresh the index."""
    stories = lane.pop("durations")
    with report.locked(directory):
        manifest: dict = report.load(directory / MANIFEST)
        if (manifest.get("schema"), manifest.get("tree"), manifest.get("profile")) != (
            report.SCHEMA,
            current,
            profile,
        ):
            manifest = {
                "schema": report.SCHEMA,
                "tree": current,
                "profile": profile,
                "lanes": {},
            }
        manifest["revision"] = archive.published(directory, current)
        manifest["lanes"][key] = lane

        # Instrumented Durations Never Feed Liveness Deadlines
        durations = report.load(directory / report.DURATIONS)
        if lane["kind"] == "test":
            suites = durations.setdefault(key, {})
            for suite, name, package, seconds in stories:
                previous = suites.get(suite, {}).get(name, {}).get("seconds")
                suites.setdefault(suite, {})[name] = {
                    "seconds": round(seconds, 3),
                    "previous": previous,
                    "package": package,
                }
        _store(directory, manifest, durations, rules)

    print(f"{key}: {lane['outcome']}, {lane['tests']} stories, {lane['failed']} failed, {lane['seconds']:.1f} s")
    return 0 if lane["outcome"] == "passed" else 1


def _store(directory: Path, manifest: dict, durations: dict, rules: policy.Policy) -> None:
    """Write the manifest, durations, merged JUnit and index; the caller holds the lock."""
    tests = directory / report.TESTS
    reports = [
        (tests / lane["junit"]).read_text(encoding="utf-8") for lane in manifest["lanes"].values() if (tests / lane["junit"]).is_file()
    ]
    report.dump(directory / MANIFEST, manifest)
    report.dump(directory / report.DURATIONS, durations)
    report.write(directory / report.JUNIT, report.merge(reports))
    report.write(directory / report.INDEX, report.index(directory, rules))


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
) -> tuple[list[report.Bench], int]:
    # Build Bench Executables Once, Then Measure Each Process Alone
    built = subprocess.run(
        [
            "cargo",
            "bench",
            "--locked",
            "--workspace",
            "--no-run",
            "--message-format",
            "json",
        ],
        cwd=directory,
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=False,
    )
    if built.returncode != 0:
        sys.stderr.write(built.stderr)
        raise policy.TestError("benchmark build failed")
    targets = []
    for line in built.stdout.splitlines():
        message = json.loads(line) if line.startswith("{") else {}
        target = message.get("target", {})
        name = target.get("name", "")
        if message.get("reason") == "compiler-artifact" and "bench" in target.get("kind", []) and message.get("executable"):
            if name == category or name.startswith(category + "-"):
                targets.append((name, message["executable"], Path(message["manifest_path"]).parent))

    home = directory / CRITERION
    baselines = _criterion_baselines(home)
    flags = ["--save-baseline", BASELINE] if refresh or not baselines else ["--baseline-lenient", BASELINE]
    environment = dict(os.environ, CRITERION_HOME=str(home))
    rows: list[report.Bench] = []
    status = 0
    for name, executable, package in targets:
        since = time.time()
        code, peak = _measured([executable, "--bench", *flags, *extra], package, environment)
        status = status or code
        domain = name.removeprefix(category).removeprefix("-") or name
        rows += _criterion_rows(home, category, domain, since, peak if measure else None, baselines)
    return rows, status


def _criterion_baselines(home: Path) -> dict[Path, float]:
    """Median of every benchmark's `dev` baseline before this run replaces anything."""
    medians = {}
    for path in home.rglob(f"{BASELINE}/estimates.json") if home.is_dir() else []:
        medians[path.parent.parent] = json.loads(path.read_text(encoding="utf-8"))["median"]["point_estimate"]
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
) -> tuple[list[report.Bench], int]:
    scratch = _scratch(directory, f"bench-{category}-python")
    output = scratch / "benchmarks.json"
    cwd = directory / project
    status, peak = _measured(pytest.bench(cwd, category, output, extra), cwd, pytest.environment(directory))
    if not output.is_file():
        raise policy.TestError("pytest-benchmark wrote no results")

    # The Baseline Is This Runner's Own Record Of Medians
    stored = directory / report.BENCHMARKS / f"python-{category}-{BASELINE}.json"
    baselines = report.load(stored)
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
        report.dump(stored, {row.name: row.median_ns for row in rows})
    return rows, status


def _percentile(values: Sequence[float], quantile: float) -> float | None:
    if len(values) < 2:
        return values[0] if values else None
    return statistics.quantiles(values, n=100, method="inclusive")[round(quantile * 100) - 1]


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
