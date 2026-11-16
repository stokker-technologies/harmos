# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Test Evidence Operations."""

# === Imports === #

# Standard Library Imports
import shutil
import time
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.test.execution

from .. import temporary
from ..build import paths
from . import archive, policy, report

# Type Checking Imports

# === Public Exports === #

# === Constants === #

SCRIPTS = ("typescript", "javascript")

# === Types === #

# === Classes === #

# === Public Functions === #


def preflight(
    directory: Path,
    category: str | None,
    language: str,
    profile: str,
) -> None:
    """Refuse a stale fixture stamp before any Rust test runs."""
    if language == "rust" and (category != "unit" or profile == "bench"):
        output = paths.output(directory, "build", profile).relative_to(directory)
        archive.fixtures(directory, profile, roots=[(output / part).as_posix() for part in ("tests", "fixtures")])


def scratch(directory: Path, key: str, profile: str) -> temporary.Directory:
    if not key.startswith("bench-"):
        output = report.location(directory, profile, key)
        if output.is_symlink() or not output.resolve().is_relative_to(directory.resolve()):
            raise policy.TestError("lane output escapes the repository")
        shutil.rmtree(output, ignore_errors=True)
    return temporary.Directory()


def collect(
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
    scratch: Path,
) -> dict:
    """Execute a lane command and turn its JUnit report into a lane record."""
    started = time.time()
    if language in SCRIPTS:
        status = harmos_tools.test.execution.execute(command, cwd, environment, timeout_s=rules.deadline(category))
    else:
        status = harmos_tools.test.execution.execute(command, cwd, environment)
    seconds = time.time() - started
    destination = report.location(directory, profile, key)
    if (
        not junit.resolve().is_relative_to(scratch.resolve())
        or any(path.is_symlink() for path in (junit, *junit.parents) if path.is_relative_to(scratch))
        or destination.is_symlink()
        or not destination.resolve().is_relative_to(directory.resolve())
    ):
        raise policy.TestError(f"{key}: native evidence escapes its owned report paths")
    for evidence in junit.parent.rglob("*"):
        if evidence.is_symlink() or not evidence.resolve().is_relative_to(junit.parent.resolve()):
            raise policy.TestError(f"{key}: native evidence escapes its report directory")
        if evidence == junit or not evidence.is_file():
            continue
        target = destination / evidence.relative_to(junit.parent)
        if target.is_symlink() or not target.resolve().is_relative_to(destination.resolve()):
            raise policy.TestError(f"{key}: native evidence destination escapes its lane")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(evidence, target)
    if not junit.is_file():
        raise policy.TestError(f"{key}: the runner exited {status} without a JUnit report")

    # Stories And Measurements
    text = junit.read_text(encoding="utf-8")
    package = Path(project).name if project != "." else directory.name
    found = report.cases(text, category, language, package)
    failed = sum(case.outcome == "failed" for case in found)
    passed = sum(case.outcome == "passed" for case in found)

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
        "seconds": round(seconds, 3),
        "started_at": int(started),
        "junit": (destination / "junit.xml").relative_to(directory).as_posix(),
        "coverage": None,
        "durations": [(case.suite, case.name, case.package, case.seconds) for case in found if case.outcome == "passed"],
    }


# === Private Functions === #

# === Entry Point === #
