# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""An Installed Wheel Serves the Public Conformance Fixture"""

# === Imports === #

# Standard Library Imports
import asyncio.subprocess
import os
import shutil
import sys
from asyncio import create_subprocess_exec, wait_for
from pathlib import Path
from subprocess import PIPE, STDOUT, run
from tempfile import mkdtemp
from unittest import IsolatedAsyncioTestCase

from harmos_tools.build.paths import output
from harmos_tools.process import spawn_flags

from tests.support import host

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class PackagedConformance(IsolatedAsyncioTestCase):
    async def test_installed_wheel_serves_the_public_fixture(self) -> None:
        root = Path(__file__).resolve().parents[4]
        scratch = Path(mkdtemp())
        self.addCleanup(shutil.rmtree, scratch)
        # Consume the Published Wheel Without Test Fixtures.
        profile = os.environ.get("TEST_PROFILE", "dev")
        wheels = sorted((output(root, "build", profile, "any") / "python").glob("harmos_sdk-*.whl"))
        if not wheels:
            raise FileNotFoundError(f"Install Python SDK: missing {profile} wheel; run mise -C sdk/python run build -- --profile {profile}")
        wheel = wheels[-1]
        environment = dict(os.environ)
        for variable in ("VIRTUAL_ENV", "PYTHONPATH", "UV_PROJECT_ENVIRONMENT"):
            environment.pop(variable, None)
        result = run(
            ["uv", "venv", "--python", sys.executable, str(scratch / "env")],
            env=environment,
            stdout=PIPE,
            stderr=STDOUT,
            text=True,
            creationflags=spawn_flags(),
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        python = scratch / "env" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        result = run(
            ["uv", "pip", "install", "--no-cache", "--python", str(python), str(wheel)],
            env=environment,
            stdout=PIPE,
            stderr=STDOUT,
            text=True,
            creationflags=spawn_flags(),
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        # Load Test-Owned Fixture Support Against Only the Installed SDK.
        shutil.copytree(root / "sdk/python/tests/fixtures/conformance", scratch / "conformance")
        source = scratch / "plugin.py"
        source.write_text(
            "import asyncio\n"
            "import sys\n"
            "from conformance import plugin\n"
            "async def main():\n"
            "    endpoint = await plugin().start('127.0.0.1:0')\n"
            "    print(endpoint.address, flush=True)\n"
            "    try:\n"
            "        await asyncio.to_thread(sys.stdin.read)\n"
            "    finally:\n"
            "        await endpoint.close()\n"
            "asyncio.run(main())\n",
            encoding="utf-8",
        )
        # Own the Plugin Lifetime Through Its Input Pipe.
        process = await create_subprocess_exec(
            str(python),
            "-E",
            "-s",
            str(source),
            cwd=scratch,
            env=environment,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            creationflags=spawn_flags(),
        )
        try:
            assert process.stdout is not None
            address = (await wait_for(process.stdout.readline(), 30)).decode().strip()
            # Run Every Reference-Host Rule Against the Installed Package.
            status, stdout, stderr = await host.run(address)
            self.assertEqual(status, 0, stdout + stderr)
        finally:
            # Close Input and Drain Both Pipes Before Removing the Environment.
            stdout, stderr = await process.communicate()
        self.assertEqual(process.returncode, 0, stdout.decode() + stderr.decode())


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
