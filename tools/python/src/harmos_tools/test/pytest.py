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
    """One pytest lane with a terminating per-story hang guard."""
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
    """Coverage arguments whose measured sources the project's own coverage configuration declares.

    The project's `pyproject.toml` names what is measured and maps installed packages back to
    their source tree, so tests that import an installed wheel still report authored modules.
    """
    configuration = project / "pyproject.toml"
    if not configuration.is_file():
        raise policy.TestError(f"missing coverage configuration {configuration}")
    return [
        "--cov",
        f"--cov-config={configuration}",
        "--cov-report=term",
        f"--cov-report=xml:{cobertura}",
    ]


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
