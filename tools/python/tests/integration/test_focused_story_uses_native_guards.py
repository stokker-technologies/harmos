# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Select Changed Stories And Product Consumers Without Building Other Targets."""

# === Imports === #

# Standard Library Imports
import tempfile
import tomllib
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.test import focused, nextest, policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class TestSelection(unittest.TestCase):
    def test_focused_story_keeps_deadlines_and_does_not_build_an_archive(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            rules = policy.Policy({}, dict(policy.DEADLINES), ())
            commands = []

            def execute(command: list[str], **options: object) -> SimpleNamespace:
                commands.append(command)
                config = Path(command[command.index("--config-file") + 1]).read_text()
                configured = tomllib.loads(config)["profile"]["project"]["overrides"]
                integration = next(item for item in configured if item["filter"] == nextest.layer("integration", ["integration"]))
                self.assertEqual(integration["slow-timeout"]["period"], f"{rules.deadline('integration')}s")
                environment = options["env"]
                assert isinstance(environment, dict)
                self.assertEqual(environment["CARGO_TARGET_DIR"], str(directory / "target"))
                junit = Path(tomllib.loads(config)["store"]["dir"]) / "project/junit.xml"
                junit.parent.mkdir(parents=True, exist_ok=True)
                junit.write_text('<testsuites><testsuite><testcase name="story"/></testsuite></testsuites>')
                return SimpleNamespace(returncode=0)

            with patch.object(focused.policy, "load", return_value=rules), patch.object(focused.subprocess, "run", side_effect=execute):
                result = focused.run(directory, package="model", target="integration", story="save_and_reopen", features="optional")
            self.assertEqual(result, 0)
            self.assertEqual(len(commands), 1)
            command = commands[0]
            self.assertEqual(command[:8], ["cargo", "nextest", "run", "--locked", "-p", "model", "--test", "integration"])
            self.assertEqual(command[-2:], ["-E", "test(/^save_and_reopen::/)"])
            self.assertEqual(command[command.index("--no-tests") + 1], "fail")
            self.assertNotIn("archive", command)
            self.assertNotIn("--workspace", command)
            self.assertNotIn("clippy", command)

            # A Package Selector Cannot Redirect Report Cleanup Outside Its Output.
            for package in ("../outside", str(directory.parent / "outside")):
                with (
                    patch.object(focused.policy, "load", return_value=rules),
                    patch.object(focused.subprocess, "run") as execute,
                ):
                    with self.assertRaises(policy.TestError):
                        focused.run(directory, package=package, target="integration", story="save_and_reopen")
                execute.assert_not_called()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
