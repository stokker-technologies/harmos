# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Test Benchmark Operations."""

# === Imports === #

# Standard Library Imports
import hashlib
import json
import math
import os
import platform
import shutil
import sys
import time
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.release.policy
import harmos_tools.test.evidence
import harmos_tools.test.execution

from ..build import paths
from . import archive, policy, pytest, report

# Type Checking Imports

# === Public Exports === #

# === Constants === #

CRITERION = report.BENCHMARKS / "criterion"
BASELINE = "dev"

# === Types === #

# === Classes === #

# === Public Functions === #


def run(
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
    harmos_tools.test.evidence.preflight(directory, category, language, "bench")
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
        rows, status = rust(directory, category, refresh, measure, extra, archive_arguments, identity)
    elif language == "python":
        identity["python"] = {
            "version": sys.version,
            "executable": archive._digest(Path(sys.executable)),
        }
        rows, status = python(directory, category, project, refresh, measure, extra, identity)
    else:
        raise policy.TestError(
            "benchmarks run for rust and python; Select Rust or Python benchmarks and rebuild their native benchmark artifacts"
        )

    from ..release import reports

    revision = os.environ.get("GITHUB_SHA") or os.environ.get("CI_COMMIT_SHA") or archive.revision(directory) or "unknown"
    measurements = reports.measurements(rows, f"{category}-{language}", revision, rules.bench_regression_percent)
    harmos_tools.release.policy.write_json(directory / report.BENCHMARKS / f"benchmarks-{category}-{language}.json", measurements)
    if not rows:
        raise policy.TestError(
            f"no {category} {language} benchmark produced results; "
            "Select Rust or Python benchmarks and rebuild their native benchmark artifacts"
        )

    for name, median, baseline in rows:
        if baseline is None:
            print(f"benchmark baseline missing: {name}")
            continue
        change = (median - baseline) / baseline * 100
        if math.isfinite(change) and change > rules.bench_regression_percent:
            print(f"benchmark regression: {name}: {change:.1f}%")
    print(f"bench {category}-{language}: {len(rows)} native results; see {report.BENCHMARKS}")
    return 0 if status == 0 else 1


def rust(
    directory: Path,
    category: str,
    refresh: bool,
    measure: bool,
    extra: Sequence[str],
    archive_arguments: Sequence[str],
    identity: dict,
) -> tuple[list[tuple[str, float, float | None]], int]:
    messages = paths.output(directory, "build", "bench") / "bench/compiler.json"
    if not messages.is_file():
        raise policy.TestError("missing benchmark compiler metadata; run mise run build -- --artifact bench first")
    archive.fixtures(directory, "bench", [messages.relative_to(directory).as_posix()])
    compatibility = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    rows = []
    status = 0
    for line in messages.read_text(encoding="utf-8").splitlines():
        message = json.loads(line) if line.startswith("{") else {}
        target = message.get("target", {})
        name = target.get("name", "")
        artifact = message.get("reason") == "compiler-artifact" and "bench" in target.get("kind", []) and message.get("executable")
        # Storage Workloads Have A Dedicated Native Nightly Job.
        if name in {"integration-multi-day", "integration-durable-lag"}:
            continue
        if not artifact or not (name == category or name.startswith(category + "-")):
            continue
        manifest = Path(message["manifest_path"])
        owner = manifest.parent / name
        if manifest.is_absolute() or ".." in owner.parts:
            raise policy.TestError(
                "benchmark owner escapes the repository; Select Rust or Python benchmarks and rebuild their native benchmark artifacts"
            )
        home = report.native_directory(directory, CRITERION / compatibility / owner)
        if any(path.is_symlink() or path.is_junction() for path in home.rglob("*")):
            raise policy.TestError(
                "native benchmark files must not be linked; Select Rust or Python benchmarks and rebuild their native benchmark artifacts"
            )
        baselines = criterion_baselines(home)
        since = time.time()
        environment = dict(os.environ, CRITERION_HOME=str(home.resolve()))
        environment.pop("STKR_TEST_REPORT_DIR", None)
        code, peak = harmos_tools.test.execution.measured(
            [str(directory / message["executable"]), "--bench", "--baseline-lenient", BASELINE, *extra],
            directory / manifest.parent,
            environment,
        )
        status = status or code
        if any(path.is_symlink() or path.is_junction() for path in home.rglob("*")):
            raise policy.TestError(
                "native benchmark files must not be linked; Select Rust or Python benchmarks and rebuild their native benchmark artifacts"
            )
        for path in sorted(home.rglob("new/estimates.json")) if home.is_dir() else []:
            if path.stat().st_mtime < since:
                continue
            # Read Structured Input
            text = path.with_name("benchmark.json").read_text(encoding="utf-8")
            benchmark = json.loads(text)
            # Read Structured Input
            text = path.read_text(encoding="utf-8")
            estimates = json.loads(text)
            key = path.parent.parent
            rows.append((benchmark.get("full_id", key.name), estimates["median"]["point_estimate"], baselines.get(key)))
            if code == 0 and (refresh or key not in baselines):
                shutil.copytree(path.parent, key / BASELINE, dirs_exist_ok=True)
        if measure and peak is not None:
            print(f"benchmark peak memory: {owner.as_posix()}: {peak} bytes")
    return rows, status


def criterion_baselines(home: Path) -> dict[Path, float]:
    medians = {}
    for path in home.rglob(f"{BASELINE}/estimates.json") if home.is_dir() else []:
        try:
            median = json.loads(path.read_text(encoding="utf-8"))["median"]["point_estimate"]
        except (OSError, ValueError, KeyError, TypeError):
            continue
        if not isinstance(median, bool) and isinstance(median, (int, float)) and math.isfinite(median) and median > 0:
            medians[path.parent.parent] = median
    return medians


def python(
    directory: Path,
    category: str,
    project: str,
    refresh: bool,
    measure: bool,
    extra: Sequence[str],
    identity: dict,
) -> tuple[list[tuple[str, float, float | None]], int]:
    owner = Path(project)
    if owner.is_absolute() or ".." in owner.parts:
        raise policy.TestError(
            "benchmark project escapes the repository; Select Rust or Python benchmarks and rebuild their native benchmark artifacts"
        )
    compatibility = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    home = report.native_directory(directory, report.BENCHMARKS / "python" / compatibility / owner / category)
    output = home / "new.json"
    stored = home / f"{BASELINE}.json"
    if output.is_symlink() or stored.is_symlink():
        raise policy.TestError(
            "native benchmark files must not be linked; Select Rust or Python benchmarks and rebuild their native benchmark artifacts"
        )
    try:
        previous = json.loads(stored.read_text(encoding="utf-8")) if stored.is_file() else {}
        baselines = {entry.get("fullname", entry["name"]): entry["stats"]["median"] * 1e9 for entry in previous.get("benchmarks", [])}
    except (OSError, ValueError, KeyError, TypeError):
        baselines = {}
    baselines = {
        name: value
        for name, value in baselines.items()
        if not isinstance(value, bool) and isinstance(value, (int, float)) and math.isfinite(value) and value > 0
    }
    output.unlink(missing_ok=True)
    cwd = directory / project
    environment = pytest.environment(directory)
    environment.pop("STKR_TEST_REPORT_DIR", None)
    status, peak = harmos_tools.test.execution.measured(pytest.bench(cwd, category, output, extra), cwd, environment)
    if not output.is_file():
        raise policy.TestError(
            "pytest-benchmark wrote no results; Select Rust or Python benchmarks and rebuild their native benchmark artifacts"
        )
    rows = []
    for entry in json.loads(output.read_text(encoding="utf-8"))["benchmarks"]:
        name = entry.get("fullname", entry["name"])
        rows.append((name, entry["stats"]["median"] * 1e9, baselines.get(name)))
    if status == 0 and (refresh or not baselines):
        shutil.copyfile(output, stored)
    if measure and peak is not None:
        print(f"benchmark peak memory: {project}/{category}: {peak} bytes")
    return rows, status


# === Private Functions === #

# === Entry Point === #
