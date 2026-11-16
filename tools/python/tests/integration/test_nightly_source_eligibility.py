# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Nightly Source Eligibility."""

# === Imports === #

# Standard Library Imports
import os
import unittest
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.release import nightly, policy

# Type Checking Imports


# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #


class NightlyTests(unittest.TestCase):
    def test_source_eligibility(self) -> None:
        base = {"GITHUB_SHA": "a" * 40, "GITHUB_RUN_ID": "41", "GITHUB_REPOSITORY": "owner/repo", "GITHUB_SERVER_URL": "https://github.com"}
        for event, ref, eligible in (
            ("schedule", "refs/heads/dev", True),
            ("workflow_dispatch", "refs/heads/dev", True),
            ("workflow_dispatch", "refs/heads/feature", False),
            ("workflow_run", "refs/heads/dev", False),
            ("push", "refs/heads/dev", False),
        ):
            with (
                self.subTest(event=event, ref=ref),
                patch.dict(os.environ, {**base, "GITHUB_EVENT_NAME": event, "GITHUB_REF": ref}, clear=True),
            ):
                if eligible:
                    source = nightly._github_source()
                    self.assertEqual(source, ("a" * 40, "41", "https://github.com/owner/repo/actions/runs/41"))
                else:
                    with self.assertRaises(policy.ToolError):
                        nightly._github_source()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
