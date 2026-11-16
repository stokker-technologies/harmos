# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Release Rejects Unpublished Pins."""

# === Imports === #

# Standard Library Imports
import json
import os
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
import harmos_tools.release.crates
from harmos_tools.release import pins, policy

# Type Checking Imports


# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[4]


# === Types === #

# === Classes === #


class PinTests(unittest.TestCase):
    def test_release_and_nightly(self) -> None:
        packages = [
            {"name": "direct", "source": "git+https://example.test/direct#123"},
            {"name": "transitive", "source": "git+https://example.test/transitive#456"},
        ]
        with patch.object(policy, "execute", return_value=json.dumps({"packages": packages})), patch.dict(os.environ, {}, clear=True):
            with self.assertRaises(policy.ToolError):
                pins.check(ROOT)
            self.assertFalse(pins.check(ROOT, warn=True))
        with patch.object(policy, "execute", return_value=json.dumps({"packages": [{"name": "local", "source": None}]})):
            self.assertTrue(pins.check(ROOT))

    def test_nightly_skips(self) -> None:
        with (
            patch.object(pins, "check", return_value=False) as check,
            patch.dict(os.environ, {"GITHUB_WORKFLOW_REF": "owner/repo/.github/workflows/nightly.yml@refs/heads/dev"}, clear=True),
        ):
            harmos_tools.release.crates.rehearse(ROOT)
            self.assertTrue(check.call_args.kwargs["warn"])


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
