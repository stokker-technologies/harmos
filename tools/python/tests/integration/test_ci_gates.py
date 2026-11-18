# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Ci Gates."""

# === Imports === #

# Standard Library Imports
import runpy
import unittest

# Third Party Imports
# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #


class ReleaseChecksTests(unittest.TestCase):
    def setUp(self) -> None:
        self.guard = runpy.run_module("harmos_tools.release.checks")
        self.sha = "a" * 40
        self.ci_run: dict = {
            "id": 7,
            "check_suite_id": 9,
            "head_sha": self.sha,
            "head_branch": "dev",
            "event": "push",
            "status": "completed",
            "conclusion": "success",
        }
        self.gate: dict = {
            "id": 12,
            "name": "Check / CI / Required",
            "app": {"slug": "github-actions", "id": 8},
            "check_suite": {"id": 9},
            "status": "completed",
            "conclusion": "success",
        }
        self.runs = [self.ci_run]
        self.checks = [self.gate]
        self.rules = []
        self.statuses = []

    def pages(self, endpoint: str, key: str | None = None) -> list[dict]:
        if endpoint.startswith("actions/"):
            return self.runs
        if "/check-runs?" in endpoint:
            return self.checks
        if endpoint == "rules/branches/main":
            return self.rules
        if endpoint.endswith("/statuses"):
            return self.statuses
        raise AssertionError(endpoint)

    def test_exact_commit(self) -> None:
        self.assertTrue(self.guard["green"](self.sha, self.pages))
        self.ci_run["head_sha"] = "b" * 40
        self.assertFalse(self.guard["green"](self.sha, self.pages))
        self.ci_run["head_sha"] = self.sha
        self.gate["check_suite"]["id"] += 1
        self.assertFalse(self.guard["green"](self.sha, self.pages))

    def test_latest_failure(self) -> None:
        self.runs.append({**self.ci_run, "id": self.ci_run["id"] + 1, "conclusion": "failure"})
        with self.assertRaises(ValueError):
            self.guard["green"](self.sha, self.pages)
        self.runs[-1]["status"] = "in_progress"
        self.assertFalse(self.guard["green"](self.sha, self.pages))

    def test_required_contexts(self) -> None:
        self.rules = [{"type": "required_status_checks", "parameters": {"required_status_checks": [{"context": "security"}]}}]
        self.assertFalse(self.guard["green"](self.sha, self.pages))
        self.statuses = [{"id": 1, "context": "security", "state": "success"}]
        self.assertTrue(self.guard["green"](self.sha, self.pages))
        self.statuses.append({"id": 2, "context": "security", "state": "failure"})
        with self.assertRaises(ValueError):
            self.guard["green"](self.sha, self.pages)
        self.rules[0]["parameters"]["required_status_checks"][0]["integration_id"] = 4
        self.statuses = []
        self.checks.append({**self.gate, "name": "security", "id": 13, "app": {"id": 5}})
        self.assertFalse(self.guard["green"](self.sha, self.pages))
        self.checks[-1]["app"]["id"] = 4
        self.assertTrue(self.guard["green"](self.sha, self.pages))
        self.checks[-1]["conclusion"] = "neutral"
        with self.assertRaises(ValueError):
            self.guard["green"](self.sha, self.pages)

    def test_wait_and_timeout(self) -> None:
        self.runs = []

        def completed(_: object) -> None:
            self.runs = [self.ci_run]

        self.guard["wait"](self.sha, self.pages, sleep=completed, clock=lambda: 0)
        self.runs = []
        times = iter([0, 100000])
        with self.assertRaises(ValueError):
            self.guard["wait"](self.sha, self.pages, sleep=lambda _: None, clock=lambda: next(times))


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
