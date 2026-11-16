# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Release Tag Selection."""

# === Imports === #

# Standard Library Imports
import json
import os
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
import harmos_tools.release.policy
import harmos_tools.release.registry
import harmos_tools.release.source
import harmos_tools.release.tag
from harmos_tools.release import policy, review

# Type Checking Imports


# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[4]

OLD = "b" * 40

NEW = "a" * 40

LATER = "c" * 40


# === Types === #

# === Classes === #


class ReleaseTagTests(unittest.TestCase):
    def setUp(self) -> None:
        self.action = vars(review)
        self.project = (ROOT / "tools/project.toml").read_text(encoding="utf-8")
        self.tag = "v" + tomllib.loads(self.project)["version"]["value"]
        self.review: dict = {
            "number": 211,
            "merged": True,
            "merged_at": "2026-11-10T00:00:00Z",
            "merge_commit_sha": NEW,
            "head": {"sha": OLD, "ref": "release/" + self.tag, "repo": {"full_name": "owner/product"}},
            "base": {"ref": "dev", "repo": {"full_name": "owner/product"}},
        }
        self.environment = {
            "GITHUB_EVENT_NAME": "workflow_run",
            "GITHUB_REF": "refs/heads/dev",
            "GITHUB_SHA": LATER,
            "GITHUB_REPOSITORY": "owner/product",
            "GITHUB_WORKFLOW_REF": "owner/product/.github/workflows/release.yml@refs/heads/dev",
        }

    def command(self, *args: str) -> str:
        if args == ("git", "rev-parse", "HEAD"):
            return LATER
        if args == ("git", "ls-remote", "--tags", "origin", "refs/tags/" + self.tag):
            return ""
        if args == ("git", "merge-base", "--is-ancestor", NEW, LATER):
            return ""
        if args == ("git", "show", f"{NEW}:tools/project.toml"):
            return self.project
        raise AssertionError(args)

    def test_release_review_validation_completion_tags_target(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory()
        temporary = temporary_directory.name
        try:
            event = Path(temporary) / "event.json"
            event.write_text(json.dumps({"workflow_run": {"head_sha": OLD, "event": "pull_request", "conclusion": "success"}}))
            older = {**self.review, "merged_at": "2026-11-09T00:00:00Z", "merge_commit_sha": OLD}
            with (
                patch.dict(os.environ, {**self.environment, "GITHUB_EVENT_PATH": str(event)}),
                patch.dict(self.action["github_release_mode"].__globals__, command=self.command, pages=lambda *args: [self.review, older]),
            ):
                self.assertEqual(self.action["github_release_mode"](), {"mode": "tag", "sha": NEW})
                with patch.dict(os.environ, GITHUB_EVENT_NAME="workflow_dispatch"):
                    self.assertEqual(self.action["github_release_mode"](), {"mode": "tag", "sha": NEW})
        finally:
            temporary_directory.cleanup()

    def test_unrelated_ci_completion_skips(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory()
        temporary = temporary_directory.name
        try:
            event = Path(temporary) / "event.json"
            with (
                patch.dict(os.environ, {**self.environment, "GITHUB_EVENT_PATH": str(event)}),
                patch.dict(self.action["github_release_mode"].__globals__, command=self.command, pages=lambda *args: [self.review]),
            ):
                for completion in (
                    {"head_sha": LATER, "event": "pull_request", "conclusion": "success"},
                    {"head_sha": OLD, "event": "push", "conclusion": "success"},
                    {"head_sha": OLD, "event": "pull_request", "conclusion": "failure"},
                ):
                    with self.subTest(completion=completion):
                        event.write_text(json.dumps({"workflow_run": completion}))
                        self.assertEqual(self.action["github_release_mode"](), {"mode": "skip", "sha": ""})
        finally:
            temporary_directory.cleanup()

    def test_dispatch_without_prepared_release_prepares(self) -> None:
        with (
            patch.dict(os.environ, {**self.environment, "GITHUB_EVENT_NAME": "workflow_dispatch"}),
            patch.dict(self.action["github_release_mode"].__globals__, command=self.command, pages=lambda *args: []),
        ):
            self.assertEqual(self.action["github_release_mode"](), {"mode": "prepare", "sha": ""})
            with patch.dict(self.action["github_release_mode"].__globals__, command=lambda *args: LATER):
                self.assertEqual(self.action["github_release_mode"](), {"mode": "prepare", "sha": ""})

    def test_prepared_release_events_require_exact_merged_review(self) -> None:
        settings = {"provider": "github", "repository": "owner/product"}
        for event in ("workflow_run", "workflow_dispatch"):
            for event_sha in (NEW, LATER):
                with (
                    self.subTest(event=event, event_sha=event_sha),
                    patch.dict(os.environ, {**self.environment, "GITHUB_EVENT_NAME": event, "GITHUB_SHA": event_sha}),
                    patch.object(harmos_tools.release.policy, "execute", return_value="") as execute,
                    patch.object(harmos_tools.release.registry, "pages", return_value=[self.review]),
                    patch.object(harmos_tools.release.registry, "document", return_value=self.review),
                    patch.object(harmos_tools.release.source, "release_tag", return_value=self.tag),
                ):
                    self.assertEqual(harmos_tools.release.tag.resolve_event(ROOT, settings, NEW), self.review)
                    if event_sha == NEW:
                        execute.assert_not_called()
                    else:
                        execute.assert_called_once_with(ROOT, ["git", "merge-base", "--is-ancestor", NEW, LATER])
                        with (
                            patch.object(harmos_tools.release.policy, "execute", side_effect=policy.ToolError("not an ancestor")),
                            self.assertRaises(policy.ToolError),
                        ):
                            harmos_tools.release.tag.resolve_event(ROOT, settings, NEW)
                    for mutation in (
                        {"merge_commit_sha": LATER},
                        {"merged_at": None},
                        {"merged": False},
                        {"head": {**self.review["head"], "ref": "feature/ordinary"}},
                        {"base": {**self.review["base"], "ref": "main"}},
                        {"head": {**self.review["head"], "repo": {"full_name": "fork/product"}}},
                    ):
                        with (
                            self.subTest(mutation=mutation),
                            patch.object(harmos_tools.release.registry, "pages", return_value=[{**self.review, **mutation}]),
                            patch.object(harmos_tools.release.registry, "document", return_value={**self.review, **mutation}),
                            self.assertRaises(policy.ToolError),
                        ):
                            harmos_tools.release.tag.resolve_event(ROOT, settings, NEW)
                    for invalid in (
                        {"GITHUB_EVENT_NAME": "push"},
                        {"GITHUB_REF": "refs/heads/feature"},
                        {"GITHUB_WORKFLOW_REF": "owner/product/.github/workflows/ci.yml@refs/heads/dev"},
                    ):
                        with patch.dict(os.environ, invalid), self.assertRaises(policy.ToolError):
                            harmos_tools.release.tag.resolve_event(ROOT, settings, NEW)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
