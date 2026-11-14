# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Route Developer Selections Without Starting Archive Or Compiler Tasks."""

# === Imports === #

# Standard Library Imports
import contextlib
import io
import unittest
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.ci import tasks
from harmos_tools.test import focused

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class FocusedDispatch(unittest.TestCase):
    def test_developer_selections_reach_only_the_focused_runner(self) -> None:
        # Select a Story, Its Package and a Set of Changed Paths.
        selections = [
            ["--package", "model", "--target", "integration", "--story", "save_and_reopen"],
            ["--package", "model"],
            ["--paths", "model/src/lib.rs", "host/tests/serve.rs"],
            ["--package", "model", "--target", "lib", "--exact", "tests::encode", "--no-run", "--timings"],
        ]
        for arguments in selections:
            with (
                self.subTest(arguments=arguments),
                patch.object(focused, "run", return_value=23) as run,
                patch.object(tasks, "_task") as task,
                patch.object(tasks.cargo, "focused") as cargo,
            ):
                self.assertEqual(tasks.dispatch(["test", *arguments]), 23)
            run.assert_called_once()
            task.assert_not_called()
            cargo.assert_not_called()
        self.assertEqual(run.call_args.kwargs["exact"], "tests::encode")
        self.assertTrue(run.call_args.kwargs["no_run"])
        self.assertTrue(run.call_args.kwargs["timings"])

        # Invalid Combinations Fail Before Any Build or Archive Step.
        selections = [
            ["--package", ""],
            ["--package", "model", "--target", "integration", "--story", ""],
            ["--package", "model", "--target", "lib", "--exact", ""],
            ["--paths", "model/src/lib.rs", "--package", "model"],
            ["--paths", "model/src/lib.rs", "--exact", "case"],
            ["--package", "model", "--story", "save", "--exact", "case"],
            ["--story", "save"],
            ["--no-run"],
            ["--package", "model", "--story", "save", "--coverage"],
            ["--package", "model", "--story", "save", "--language", "python"],
        ]
        for arguments in selections:
            with (
                self.subTest(arguments=arguments),
                patch.object(focused, "run") as run,
                contextlib.redirect_stderr(io.StringIO()),
            ):
                with self.assertRaises(SystemExit) as error:
                    tasks.dispatch(["test", *arguments])
            self.assertEqual(error.exception.code, 2)
            run.assert_not_called()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
