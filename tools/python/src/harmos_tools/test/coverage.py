# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Test Coverage Operations."""

# === Imports === #

# Standard Library Imports
import os
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.test.evidence
import harmos_tools.test.execution

from . import archive, nextest, policy, pytest, report

# Type Checking Imports

# === Public Exports === #

# === Constants === #

VARIABLE = "COVERAGE"

# === Types === #

# === Classes === #

# === Public Functions === #


def run(
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
    harmos_tools.test.evidence.preflight(directory, category, language, profile)
    scratch_directory = harmos_tools.test.evidence.scratch(directory, key, profile)
    scratch = scratch_directory.path
    try:
        categories = [category] if category else list(policy.CATEGORIES)
        deadline = max(rules.deadline(name, coverage=True) for name in categories)
        cobertura = scratch / "coverage.xml"
        environment = dict(os.environ, **{VARIABLE: "1"})

        # Instrumented Run
        if language == "rust":
            binaries = archive.binaries(directory, profile)
            names = sorted(set(binaries.values()))
            expression, rest = nextest.selection(category, extra, names)
            measured = nextest.overrides(rules, category, coverage=True, binaries=binaries)
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
            environment = {**pytest.environment(directory), VARIABLE: "1"}
        else:
            raise policy.TestError("coverage is measured for rust and python")

        environment["STKR_TEST_PROFILE"] = profile
        environment.pop("STKR_TEST_REPORT_DIR", None)
        lane = harmos_tools.test.evidence.collect(
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
            scratch,
        )
        if language == "rust" and harmos_tools.test.execution.execute(nextest.report(profile, cobertura), directory, environment) != 0:
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
    finally:
        scratch_directory.close()


# === Private Functions === #

# === Entry Point === #
