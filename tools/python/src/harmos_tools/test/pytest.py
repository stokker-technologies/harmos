# Rendered by stkr project update from templates/python/test/pytest.py; sha256:44f40de3639b011b13a2b6ff7d5d93b096f4825c94e2486b474baf60affa4249
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Pytest Commands For Python Lanes, Coverage And Benchmarks.

A Python project keeps its stories under `tests/<category>/` and its benchmarks under
`benchmarks/<category>/`. Lanes run in the shared root `.venv/` without syncing it, so a
lane never builds the project: the build step installs the wheel or the editable project
first. Lane arguments after `--` are appended to pytest.
"""

# === Imports === #

# Standard Library Imports
import os
import sys
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports
# Local Imports
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

STRICT = ["--strict-config", "--strict-markers", "--import-mode=importlib"]

# === Types === #

# === Classes === #

# === Public Functions === #


def environment(directory: Path) -> dict[str, str]:
    """The shared root environment with no ambient selection that could narrow a lane."""
    variables = dict(os.environ)
    variables.pop("UV_PROJECT_ENVIRONMENT", None)
    variables.pop("PYTEST_ADDOPTS", None)
    variables.pop("COVERAGE_PROCESS_START", None)
    return variables


def run(
    project: Path,
    suite: str,
    deadline: int,
    junit: Path,
    arguments: Sequence[str],
) -> list[str]:
    """One pytest lane whose per-story deadline is the lane maximum."""
    if not (project / suite).is_dir():
        raise policy.TestError(f"missing Python suite {project / suite}")
    return [
        sys.executable,
        "-m",
        "pytest",
        *STRICT,
        "--timeout-method=thread",
        f"--timeout={deadline}",
        f"--junitxml={junit}",
        suite,
        *arguments,
    ]


def coverage(project: Path, cobertura: Path) -> list[str]:
    """Coverage arguments measuring the authored sources of the project."""
    source = project / "src"
    measured = source if source.is_dir() else project
    return [f"--cov={measured}", f"--cov-report=xml:{cobertura}"]


def bench(project: Path, category: str, output: Path, arguments: Sequence[str]) -> list[str]:
    suite = f"benchmarks/{category}"
    if not (project / suite).is_dir():
        raise policy.TestError(f"missing Python benchmarks {project / suite}")
    return [
        sys.executable,
        "-m",
        "pytest",
        *STRICT,
        "--benchmark-only",
        f"--benchmark-json={output}",
        suite,
        *arguments,
    ]


# === Private Functions === #


# === Entry Point === #
