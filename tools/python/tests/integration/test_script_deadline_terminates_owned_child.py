# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Terminate Only the Owned Child When Its Configured Hang Guard Fires."""

# === Imports === #

# Standard Library Imports
import signal
import subprocess
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

# Third Party Imports
# Local Imports
from harmos_tools.test import execution

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ScriptDeadline(unittest.TestCase):
    def test_expiry_terminates_and_reaps_the_owned_process(self) -> None:
        directory = Path.cwd()
        command = ["script", "argument"]
        timeout_s = 30
        kill_signal = getattr(signal, "SIGKILL", 9)
        for platform in ("nt", "posix"):
            # Deliver a Controlled Deadline Expiry
            child = Mock(pid=123, stdout=None, stderr=None, stdin=None)
            child.wait.side_effect = [subprocess.TimeoutExpired(command, timeout_s), 0, 0]
            child.poll.return_value = None
            with (
                patch.object(execution, "console", return_value={}),
                patch.object(execution, "spawn_flags", return_value=0),
                patch.object(execution.os, "name", platform),
                patch.object(execution.signal, "SIGKILL", kill_signal, create=True),
                patch.object(execution.os, "killpg", create=True) as kill_group,
                patch.object(execution.subprocess, "Popen", return_value=child) as start,
                patch.object(execution.subprocess, "run") as terminate_tree,
            ):
                result = execution.execute(command, directory, {}, timeout_s=timeout_s)

            # Check Child Ownership and Reaping
            self.assertEqual(result, 1)
            self.assertEqual(start.call_args.args[0], command)
            child.wait.assert_any_call(timeout=timeout_s)
            child.kill.assert_called_once_with()
            self.assertEqual(child.wait.call_args.args, ())
            if platform == "nt":
                self.assertEqual(terminate_tree.call_args.args[0], ["taskkill.exe", "/PID", str(child.pid), "/T", "/F"])
                kill_group.assert_not_called()
            else:
                kill_group.assert_called_once_with(child.pid, kill_signal)
                terminate_tree.assert_not_called()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
