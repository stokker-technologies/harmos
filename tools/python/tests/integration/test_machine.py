# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Prove Machine Lock Serialization, Crash Release And Nested Ownership"""

# === Imports === #

# Standard Library Imports
import os
import queue
import subprocess
import sys
import tempfile
import threading
import unittest
from typing import TextIO, cast

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

CHILD = """
from harmos_tools.build.machine import lock
import os, sys
with lock():
    print(f"Entered {os.getpid()}", flush=True)
    if sys.stdin.readline().strip() == "crash":
        os._exit(17)
"""

# === Types === #

# === Classes === #


class MachineLockTests(unittest.TestCase):
    def test_holders_serialize_and_dead_holder_releases(self):
        for crash in (False, True):
            with self.subTest(crash=crash), tempfile.TemporaryDirectory() as temporary:
                environment = dict(os.environ, LOCALAPPDATA=temporary, XDG_CACHE_HOME=temporary, RUSTC_WRAPPER="")
                environment.pop("STKR_NATIVE_BUILD_LOCK", None)
                environment["MISE_TASK_NAME"] = "test:holder"
                command = [sys.executable, "-u", "-c", CHILD]
                processes = []
                try:
                    first = subprocess.Popen(
                        command, env=environment, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
                    )
                    processes.append(first)
                    first_input = cast(TextIO, first.stdin)
                    entered = self.read_until(first, "Entered")
                    second = subprocess.Popen(
                        command, env=environment, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
                    )
                    processes.append(second)
                    second_input = cast(TextIO, second.stdin)
                    # This Message Follows An Actual Failed Nonblocking OS Lock Attempt.
                    waiting = self.read_until(second, "Waiting For Build Lock Held By")
                    self.assertIn(entered.split()[-1], waiting)
                    self.assertIn("test:holder", waiting)
                    if crash:
                        first_input.write("crash\n")
                        first_input.flush()
                        self.assertEqual(first.wait(timeout=30), 17)
                    else:
                        first_input.write("release\n")
                        first_input.flush()
                        self.assertEqual(first.wait(timeout=30), 0)
                    self.read_until(second, "Entered")
                    second_input.write("release\n")
                    second_input.flush()
                    self.assertEqual(second.wait(timeout=30), 0)
                finally:
                    for process in processes:
                        if process.poll() is None:
                            process.kill()
                        process.communicate(timeout=30)

    def test_nested_process_reuses_ownership(self):
        with tempfile.TemporaryDirectory() as temporary:
            environment = dict(os.environ, LOCALAPPDATA=temporary, XDG_CACHE_HOME=temporary, RUSTC_WRAPPER="")
            environment.pop("STKR_NATIVE_BUILD_LOCK", None)
            code = "from harmos_tools.build.machine import lock; import sys\nwith lock():\n print('Nested'); sys.exit(23)"
            result = subprocess.run(
                [sys.executable, "-m", "harmos_tools.build.machine", "--", "python", "-c", code],
                env=environment,
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
            self.assertEqual(result.returncode, 23, result.stdout + result.stderr)
            self.assertIn("Nested", result.stdout)

    def read_until(self, process, marker):
        result = queue.Queue()

        def read():
            while line := process.stdout.readline():
                if marker in line:
                    result.put(line)
                    return
            result.put(None)

        threading.Thread(target=read, daemon=True).start()
        line = result.get(timeout=30)
        self.assertIsNotNone(line, f"holder exited before {marker}: {process.poll()}")
        return line


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
