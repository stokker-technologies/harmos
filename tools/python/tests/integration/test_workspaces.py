# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Prove Simulation Scratch Recovery"""

# === Imports === #

# Standard Library Imports
import contextlib
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.ci import containers, workspaces

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class WorkspaceTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="harmos-workspaces-test-")
        self.addCleanup(directory.cleanup)
        self.base = Path(directory.name)
        self.output = self.base / "output"
        self.output.mkdir()
        self.enterContext(patch.object(tempfile, "gettempdir", return_value=str(self.base)))
        self.enterContext(patch.dict(os.environ, LOCALAPPDATA=str(self.base / "cache"), XDG_CACHE_HOME=str(self.base / "cache")))

    def test_crashed_owner_without_run_records_is_recovered(self):
        script = """
import os, sys, tempfile
from pathlib import Path
from harmos_tools.ci import workspaces
tempfile.tempdir = sys.argv[1]
with workspaces.job(Path(sys.argv[2]), 'repository', 'run', 'job', lambda _: None, stable=True) as owner:
    (owner.root / 'workspace').mkdir()
    (owner.root / 'workspace' / 'proof').write_text('retained diagnostic')
    os._exit(23)
"""
        result = subprocess.run([sys.executable, "-c", script, str(self.base), str(self.output)], timeout=30, check=False)
        self.assertEqual(result.returncode, 23)
        root = self.base / "project-ci-job-repository-job-v2"
        self.assertTrue(root.exists())
        shutil.rmtree(self.output)
        self.output.mkdir()
        preserved = []
        workspaces.recover(self.output, "repository", lambda _, key, path: preserved.append((key, (path / "proof").read_text())))
        self.assertEqual(preserved, [("job", "retained diagnostic")])
        self.assertFalse(root.exists())
        with workspaces.job(self.output, "repository", "next", "job", lambda _: None, stable=True) as owner:
            self.assertEqual(owner.root, root)
        self.assertFalse(root.exists())

    def test_foreign_and_legacy_directories_are_retained(self):
        for kind in ("foreign", "legacy", "malformed"):
            with self.subTest(kind=kind):
                root = self.base / f"project-ci-job-repository-{kind}"
                root.mkdir()
                (root / "proof").write_text(kind)
                if kind == "legacy":
                    (root / workspaces.MARKER).write_text(json.dumps({"repository": "repository", "root": str(root)}))
                elif kind == "malformed":
                    (root / workspaces.MARKER).write_text("null")
        with contextlib.redirect_stderr(io.StringIO()) as messages:
            workspaces.recover(self.output, "repository", lambda *_: self.fail("foreign scratch must not be preserved or deleted"))
        for kind in ("foreign", "legacy", "malformed"):
            root = self.base / f"project-ci-job-repository-{kind}"
            self.assertEqual((root / "proof").read_text(), kind)
            self.assertIn(str(root), messages.getvalue())

    def test_live_owner_is_retained(self):
        with workspaces.job(self.output, "repository", "run", "job", lambda _: None, stable=True) as owner:
            with contextlib.redirect_stderr(io.StringIO()) as messages:
                workspaces.recover(self.output, "repository", lambda *_: self.fail("live scratch must not be touched"))
            self.assertTrue(owner.root.is_dir())
            self.assertIn("busy", messages.getvalue())
            with self.assertRaises(containers.Busy), workspaces.lease(owner.root, "repository"):
                self.fail("a live lease cannot be acquired twice")
        self.assertFalse(owner.root.exists())

    def test_publication_never_replaces_a_racing_foreign_directory(self):
        publish = workspaces.publish
        root = self.base / "project-ci-job-repository-job-v2"

        def racing(staged, target):
            target.mkdir()
            publish(staged, target)

        with (
            patch.object(workspaces, "publish", side_effect=racing),
            self.assertRaises(FileExistsError),
            workspaces.job(self.output, "repository", "run", "job", lambda _: None, stable=True),
        ):
            self.fail("a racing foreign directory must not be replaced")
        self.assertTrue(root.is_dir())
        self.assertEqual(list(root.iterdir()), [])

    def test_tampered_root_is_retained(self):
        with (
            patch.object(workspaces, "cleanup"),
            workspaces.job(self.output, "repository", "run", "job", lambda _: None, stable=True) as owner,
        ):
            foreign = self.base / "project-ci-job-repository-foreign-v2"
            foreign.mkdir()
            (foreign / "proof").write_text("foreign")
            record = dict(owner.record, root=str(foreign))
            workspaces.write(owner.root / workspaces.MARKER, record)
        with contextlib.redirect_stderr(io.StringIO()):
            workspaces.recover(self.output, "repository", lambda *_: self.fail("tampered scratch must not be touched"))
        self.assertTrue(owner.root.exists())
        self.assertEqual((foreign / "proof").read_text(), "foreign")

    def test_failed_cleanup_is_a_job_failure(self):
        def preserve(_):
            raise OSError("injected preservation failure")

        with (
            contextlib.redirect_stderr(io.StringIO()),
            self.assertRaises(OSError),
            workspaces.job(self.output, "repository", "run", "job", preserve, stable=True) as owner,
        ):
            (owner.root / "workspace").mkdir()
        self.assertTrue(owner.root.exists())
        self.assertTrue((owner.root / workspaces.MARKER).is_file())

    def test_foreign_filesystem_retains_the_entire_owned_tree(self):
        with (
            patch.object(workspaces, "cleanup"),
            workspaces.job(self.output, "repository", "run", "job", lambda _: None, stable=True) as owner,
        ):
            foreign = owner.root / "foreign"
            foreign.mkdir()
            (foreign / "proof").write_text("retained")
        stat = os.stat

        def mounted(path, *args, **kwargs):
            metadata = stat(path, *args, **kwargs)
            return SimpleNamespace(st_dev=metadata.st_dev + 1) if Path(path) == foreign else metadata

        with (
            patch.object(os, "stat", side_effect=mounted),
            self.assertRaises(ValueError),
        ):
            owner.cleanup(lambda _: self.fail("foreign filesystems must be refused before preservation or deletion"))
        self.assertEqual((foreign / "proof").read_text(), "retained")
        self.assertTrue((owner.root / workspaces.MARKER).is_file())

    def test_normal_and_failing_jobs_remove_scratch(self):
        for failure in (False, True):
            with self.subTest(failure=failure):
                output = self.output / str(failure)
                output.mkdir()
                try:
                    with workspaces.job(output, "repository", "run", "job", lambda _: None, stable=True) as owner:
                        (owner.root / "payload").write_bytes(b"test fixture")
                        marker = json.loads((owner.root / workspaces.MARKER).read_text())
                        self.assertEqual(marker, owner.record)
                        if failure:
                            raise RuntimeError("injected failure")
                except RuntimeError:
                    self.assertTrue(failure)
                self.assertFalse(owner.root.exists())

    @unittest.skipUnless(os.name == "nt", "Windows named job ownership")
    def test_abandoned_windows_launch_is_recovered(self):
        with (
            patch.object(workspaces, "cleanup"),
            workspaces.job(self.output, "repository", "run", "job", lambda _: None, stable=True) as owner,
        ):
            owner.starting()
        workspaces.recover(self.output, "repository", lambda *_: None)
        self.assertFalse(owner.root.exists())

    def test_unknown_command_and_failed_preservation_retain_ownership(self):
        for reason in ("command", "empty-command", "null-command", "diagnostics"):
            with self.subTest(reason=reason):
                output = self.output / reason
                output.mkdir()
                with (
                    patch.object(workspaces, "cleanup"),
                    workspaces.job(output, "repository", reason, reason, lambda _: None, stable=True) as owner,
                ):
                    if reason != "diagnostics":
                        owner.record["command"] = {"command": {"platform": "unknown"}, "empty-command": {}, "null-command": None}[reason]
                        owner.save()

                def preserve(*_, reason=reason):
                    if reason == "diagnostics":
                        raise OSError("injected preservation failure")

                with contextlib.redirect_stderr(io.StringIO()):
                    workspaces.recover(self.output, "repository", preserve)
                self.assertTrue(owner.root.exists())
                self.assertTrue((owner.root / workspaces.MARKER).is_file())


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
