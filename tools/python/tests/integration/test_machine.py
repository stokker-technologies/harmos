# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Prove Concurrent Task Launch And Exit Propagation"""

# === Imports === #

# Standard Library Imports
import os
import queue
import subprocess
import sys
import threading
import unittest
from typing import TextIO, cast

# Third Party Imports
# Local Imports
from harmos_tools.process import spawn_flags
# Type Checking Imports

# === Public Exports === #

# === Constants === #

CHILD = "import sys; print('Entered', flush=True); sys.exit(int(sys.stdin.readline()))"

# === Types === #

# === Classes === #


class MachineTaskTests(unittest.TestCase):
    def test_tasks_enter_concurrently(self):
        environment = dict(os.environ, RUSTC_WRAPPER="")
        environment.pop("UV_PROJECT_ENVIRONMENT", None)
        command = [sys.executable, "-m", "harmos_tools.build.machine", "--", "python", "-u", "-c", CHILD]
        processes = []
        try:
            for _ in range(2):
                process = subprocess.Popen(
                    command,
                    env=environment,
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    creationflags=spawn_flags(),
                )
                processes.append(process)
                self.read_until(process, "Entered")
            # Both Children Enter Before Either Is Released.
            for process, status in zip(processes, (17, 23), strict=True):
                incoming = cast(TextIO, process.stdin)
                incoming.write(f"{status}\n")
                incoming.flush()
                self.assertEqual(process.wait(timeout=30), status)
        finally:
            for process in processes:
                if process.poll() is None:
                    process.kill()
                process.communicate(timeout=30)

    def read_until(self, process, marker):
        result = queue.Queue()

        def read():
            while line := process.stdout.readline():
                if marker in line:
                    result.put(line)
                    return
            result.put(None)

        threading.Thread(target=read, daemon=True).start()
        self.assertIsNotNone(result.get(timeout=30), f"task exited before {marker}: {process.poll()}")


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
