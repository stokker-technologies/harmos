# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Follow A Nightly Revision From Freshness To Its Owned Result Report."""

# === Imports === #

# Standard Library Imports
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

# Third Party Imports

# Local Imports
from harmos_tools.ci import tasks, workflow
from harmos_tools.release import nightly, review

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class NightlyRunTests(unittest.TestCase):
    def test_revision_is_built_once_and_receives_only_its_own_results(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "output"
            environment = {
                "GITHUB_EVENT_NAME": "schedule",
                "GITHUB_REF": "refs/heads/dev",
                "GITHUB_REPOSITORY": "owner/repo",
                "GITHUB_RUN_ID": "7",
                "GITHUB_SHA": "a" * 40,
                "GITHUB_OUTPUT": str(output),
                "RUN_URL": "https://example.test/actions/runs/7",
            }
            with (
                patch.dict(os.environ, environment, clear=True),
                patch.object(workflow.subprocess, "run") as lookup,
                patch.object(review, "request") as request,
                patch.object(review, "pages", return_value=[]) as pages,
            ):
                # Plan The First Scheduled Build Without A Published Tag.
                lookup.return_value = SimpleNamespace(returncode=1, stdout="HTTP/2.0 404", stderr="")
                self.assertEqual(tasks.main(["ci:nightly-freshness"]), 0)
                self.assertEqual(output.read_text().splitlines()[-1], "build=true")
                request.assert_not_called()

                # Skip The Revision Already Published And Build A Changed Revision.
                lookup.return_value = SimpleNamespace(returncode=0, stdout="", stderr="")
                for published, expected in (("a" * 40, False), ("b" * 40, True)):
                    request.return_value = {"object": {"sha": published}}
                    self.assertEqual(tasks.main(["ci:nightly-freshness"]), 0)
                    self.assertEqual(output.read_text().splitlines()[-1], f"build={str(expected).lower()}")

                # Refuse Unavailable Or Malformed Provider Evidence.
                previous = output.read_bytes()
                for failure in ("HTTP 403", "HTTP 429", "HTTP 500", "connection refused"):
                    lookup.return_value = SimpleNamespace(returncode=1, stdout="", stderr=failure)
                    with self.assertRaises(RuntimeError):
                        tasks.main(["ci:nightly-freshness"])
                    self.assertEqual(output.read_bytes(), previous)
                lookup.return_value = SimpleNamespace(returncode=0, stdout="", stderr="")
                request.side_effect = subprocess.CalledProcessError(1, "gh")
                with self.assertRaises(subprocess.CalledProcessError):
                    tasks.main(["ci:nightly-freshness"])
                request.side_effect = None

                # Dispatch Manually And Suppress An Already Running Or Queued Revision.
                os.environ["GITHUB_EVENT_NAME"] = "workflow_dispatch"
                lookup.reset_mock()
                for status in ("in_progress", "queued"):
                    pages.side_effect = lambda endpoint, key, status=status: [{"id": 8}] if f"status={status}" in endpoint else []
                    self.assertEqual(tasks.main(["ci:nightly-freshness"]), 0)
                    self.assertEqual(output.read_text().splitlines()[-1], "build=false")
                pages.side_effect = None
                pages.return_value = [{"id": 7}]
                self.assertEqual(tasks.main(["ci:nightly-freshness"]), 0)
                self.assertEqual(output.read_text().splitlines()[-1], "build=true")
                lookup.assert_not_called()
                pages.side_effect = subprocess.CalledProcessError(1, "gh")
                with self.assertRaises(subprocess.CalledProcessError):
                    tasks.main(["ci:nightly-freshness"])
                os.environ["GITHUB_REF"] = "refs/heads/feature"
                with self.assertRaises(ValueError):
                    tasks.main(["ci:nightly-freshness"])

            # Publish Through The Same Channel-Aware Entry Point As Stable Releases.
            with patch.object(nightly, "publish") as publish:
                self.assertEqual(tasks.main(["release:publish", "--channel", "nightly"]), 0)
                publish.assert_called_once_with(Path.cwd())


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
