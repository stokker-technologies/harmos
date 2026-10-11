# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Benchmark the complete simulation determinism oracle."""

# === Imports === #

# Standard Library Imports
import subprocess
import sys
from pathlib import Path

# Third Party Imports
from pytest_benchmark.fixture import BenchmarkFixture

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def test_double_run_identity(benchmark: BenchmarkFixture) -> None:
    root = Path(__file__).resolve().parents[4]
    benchmark(
        subprocess.run,
        [sys.executable, "-m", "harmos_tools.bench"],
        cwd=root,
        check=True,
    )


# === Private Functions === #

# === Entry Point === #
