# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Python Registry Publication."""

# === Imports === #

# Standard Library Imports
import contextlib
import email.message
import hashlib
import io
import json
import os
import tempfile
import unittest
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
import harmos_tools.release.python_registry
import harmos_tools.release.rehearse
import harmos_tools.release.requirements
import harmos_tools.release.source
import harmos_tools.task.execute
from harmos_tools.ci import tasks
from harmos_tools.release import policy, registry

# Type Checking Imports


# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #


class PythonPublicationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.package = self.root / "target/build/release/any/python/fixture-1.0-py3-none-any.whl"
        self.package.parent.mkdir(parents=True)
        self.package.write_bytes(b"wheel")
        project = self.root / "sdk/python/pyproject.toml"
        project.parent.mkdir(parents=True)
        project.write_text('[project]\nname = "fixture"\n')

    def test_pypi_resumes_only_matching_files_and_rejects_mismatches(self) -> None:
        for digest in (None, hashlib.sha256(b"wheel").hexdigest(), hashlib.sha256(b"different").hexdigest()):
            files = [] if digest is None else [{"filename": self.package.name, "digests": {"sha256": digest}}]
            response = io.BytesIO(json.dumps({"releases": {"1.0": files}}).encode())
            with (
                self.subTest(digest=digest),
                patch.dict(os.environ, {"RELEASE_PROFILE": "release"}, clear=True),
                patch.object(registry.urllib.request, "urlopen", return_value=response),
                patch.object(policy, "execute") as execute,
            ):
                if digest and digest != hashlib.sha256(b"wheel").hexdigest():
                    with self.assertRaises(policy.ToolError):
                        harmos_tools.release.python_registry.publish(self.root, {"provider": "github"}, dry_run=False)
                    execute.assert_not_called()
                else:
                    harmos_tools.release.python_registry.publish(self.root, {"provider": "github"}, dry_run=False)
                    if digest:
                        execute.assert_not_called()
                    else:
                        execute.assert_called_once_with(
                            self.root,
                            [
                                "uv",
                                "publish",
                                "--trusted-publishing",
                                "always",
                                "--check-url",
                                "https://pypi.org/simple/",
                                str(self.package.relative_to(self.root)),
                            ],
                        )

    def test_partial_upload_resumes_and_checks_all_files_before_uploading(self) -> None:
        missing = self.package.with_name("fixture-1.0.tar.gz")
        missing.write_bytes(b"source")
        for digest in (hashlib.sha256(b"wheel").hexdigest(), hashlib.sha256(b"changed").hexdigest()):
            published = {"releases": {"1.0": [{"filename": self.package.name, "digests": {"sha256": digest}}]}}
            with (
                self.subTest(digest=digest),
                patch.dict(os.environ, {"RELEASE_PROFILE": "release"}, clear=True),
                patch.object(registry.urllib.request, "urlopen", return_value=io.BytesIO(json.dumps(published).encode())),
                patch.object(policy, "execute") as execute,
            ):
                if digest == hashlib.sha256(b"wheel").hexdigest():
                    harmos_tools.release.python_registry.publish(self.root, {"provider": "github"}, dry_run=False)
                    self.assertEqual(execute.call_args.args[1][-1], str(missing.relative_to(self.root)))
                    self.assertNotIn(str(self.package.relative_to(self.root)), execute.call_args.args[1])
                else:
                    with self.assertRaises(policy.ToolError):
                        harmos_tools.release.python_registry.publish(self.root, {"provider": "github"}, dry_run=False)
                    execute.assert_not_called()

    def test_first_pypi_project_and_lookup_failure(self) -> None:
        for status in (404, 403, 500):
            error = urllib.error.HTTPError("https://pypi.org", status, "error", email.message.Message(), None)
            self.addCleanup(error.close)
            with (
                self.subTest(status=status),
                patch.dict(os.environ, {"RELEASE_PROFILE": "release"}, clear=True),
                patch.object(registry.urllib.request, "urlopen", side_effect=error),
                patch.object(policy, "execute") as execute,
            ):
                if status == 404:
                    harmos_tools.release.python_registry.publish(self.root, {"provider": "github"}, dry_run=False)
                    execute.assert_called_once()
                else:
                    with self.assertRaises(urllib.error.HTTPError):
                        harmos_tools.release.python_registry.publish(self.root, {"provider": "github"}, dry_run=False)
                    execute.assert_not_called()

    def test_python_dry_run_is_offline_and_credential_free(self) -> None:
        environment = {"RELEASE_PROFILE": "release", "UV_PUBLISH_TOKEN": "secret", "ACTIONS_ID_TOKEN_REQUEST_URL": "https://oidc"}
        observed = []
        with (
            patch.dict(os.environ, environment, clear=True),
            patch.object(registry.urllib.request, "urlopen") as network,
            patch.object(policy, "execute", side_effect=lambda *args: observed.append(dict(os.environ))) as execute,
        ):
            harmos_tools.release.python_registry.publish(self.root, {"provider": "github"}, dry_run=True)
            self.assertEqual(os.environ["UV_PUBLISH_TOKEN"], "secret")
        network.assert_not_called()
        execute.assert_called_once_with(
            self.root, ["uv", "publish", "--dry-run", "--trusted-publishing", "never", str(self.package.relative_to(self.root))]
        )
        self.assertNotIn("UV_PUBLISH_TOKEN", observed[0])
        self.assertNotIn("ACTIONS_ID_TOKEN_REQUEST_URL", observed[0])

    def test_rehearsal_includes_python_without_a_task_presence_guard(self) -> None:
        with (
            patch.object(harmos_tools.release.source, "continuous_integration", return_value=False),
            patch.object(harmos_tools.release.requirements, "refuse_overrides"),
            patch.object(harmos_tools.release.rehearse, "dry"),
            patch.object(harmos_tools.release.source, "task_defined", return_value=False),
            patch.object(harmos_tools.release.python_registry, "publish") as python,
        ):
            settings = {"targets": ["fixture"]}
            harmos_tools.release.rehearse.run(self.root, settings, target="fixture")
        python.assert_called_once_with(self.root, settings, dry_run=True)

    def test_dispatch_keeps_pypi_separate_from_assets(self) -> None:
        for dry in (False, True):
            with (
                patch.dict(os.environ, {}, clear=True),
                patch.object(harmos_tools.task.execute, "module", return_value=0) as module,
                patch.object(harmos_tools.task.execute, "task", return_value=0) as task,
            ):
                arguments = ["release:publish", "--channel", "stable", "--registry", "pypi"]
                self.assertEqual(tasks.dispatch([*arguments, *(["--dry"] if dry else [])]), 0)
                module.assert_called_once_with("release.run", "python-dry" if dry else "python")
                task.assert_not_called()
        with (
            patch.dict(os.environ, {}, clear=True),
            patch.object(harmos_tools.task.execute, "task", return_value=0) as task,
            patch.object(harmos_tools.task.execute, "module") as module,
        ):
            tasks.dispatch(["release:publish", "--channel", "stable"])
            self.assertEqual([call.args for call in task.call_args_list], [("release:publish:assets",), ("release:publish:finalize",)])
            module.assert_not_called()

    def test_pypi_rejects_nightly_and_stage_selection(self) -> None:
        for arguments in (["--channel", "nightly"], ["--stage", "assets"]):
            with self.subTest(arguments=arguments), contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                tasks.dispatch(["release:publish", "--registry", "pypi", *arguments])


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
