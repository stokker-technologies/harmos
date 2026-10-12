# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Prepare Native And Guest Acceptance Artifacts Once Per Session"""

# === Imports === #

# Standard Library Imports
from pathlib import Path

# Third Party Imports
import pytest

# Local Imports
from harmos_tools.packages import verify

# Type Checking Imports


# === Public Exports === #

# === Constants === #
ROOT = Path(__file__).resolve().parents[4]

# === Types === #

# === Classes === #

# === Public Functions === #


@pytest.fixture(scope="session")
def native_examples() -> dict[str, Path]:
    return verify(ROOT)


@pytest.fixture(scope="session")
def built_examples(native_examples: dict[str, Path]) -> dict[str, Path]:
    return native_examples


# === Private Functions === #

# === Entry Point === #
