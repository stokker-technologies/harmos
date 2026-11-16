# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Test Lane Operations."""

# === Imports === #

# Standard Library Imports
import os
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.test.evidence

from ..build import paths
from . import archive, nextest, policy, pytest, report, vitest

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(
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
    harmos_tools.test.evidence.preflight(directory, category, language, profile)
    if language == "python":
        archive.install_wheels(directory, profile)
    scratch_directory = harmos_tools.test.evidence.scratch(directory, key, profile)
    scratch = scratch_directory.path
    try:
        deadline = rules.deadline(category)

        # Native Command
        environment = dict(os.environ)
        cwd = directory / project
        if language == "rust":
            path = archive.require(directory, profile)
            binaries = archive.binaries(directory, profile)
            names = sorted(set(binaries.values()))
            expression, rest = nextest.selection(category, extra, names)
            measured = nextest.overrides(rules, category, coverage=False, binaries=binaries)
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

        environment["STKR_TEST_PROFILE"] = profile
        environment.pop("STKR_TEST_REPORT_DIR", None)
        if category == "system":
            traces = report.native_directory(directory, report.TRACES / profile / paths.target() / key)
            environment["STKR_TEST_REPORT_DIR"] = str(traces)
        lane = harmos_tools.test.evidence.collect(
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
            scratch,
        )
        lane["arguments"] = list(extra)
        return 0 if lane["outcome"] == "passed" else 1
    finally:
        scratch_directory.close()


# === Private Functions === #

# === Entry Point === #
