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
import asyncio
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from harmos_tools.process import spawn_flags

from tests.support import host

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class PackagedConformance(unittest.IsolatedAsyncioTestCase):
    async def test_installed_wheel_serves_the_public_fixture(self) -> None:
        root = Path(__file__).resolve().parents[4]
        with tempfile.TemporaryDirectory() as name:
            scratch = Path(name)
            # Build the Published Wheel Without Test Fixtures.
            result = subprocess.run(
                ["mise", "run", "build", "--", "--package", "harmos-sdk"],
                cwd=root,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            self.assertEqual(result.returncode, 0, result.stdout)
            wheel = next((root / "target/build/dev/any/python").glob("harmos_sdk-*.whl"))
            environment = dict(os.environ)
            for variable in ("VIRTUAL_ENV", "PYTHONPATH", "UV_PROJECT_ENVIRONMENT"):
                environment.pop(variable, None)
            result = subprocess.run(
                ["uv", "venv", "--python", sys.executable, str(scratch / "env")],
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            self.assertEqual(result.returncode, 0, result.stdout)
            python = scratch / "env" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
            result = subprocess.run(
                ["uv", "pip", "install", "--offline", "--python", str(python), str(wheel)],
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            self.assertEqual(result.returncode, 0, result.stdout)
            # Load Test-Owned Fixture Support Against Only the Installed SDK.
            shutil.copytree(root / "sdk/python/tests/fixtures/conformance", scratch / "conformance")
            source = scratch / "plugin.py"
            source.write_text(
                "import asyncio\nimport sys\nfrom pathlib import Path\nsys.path.insert(0, str(Path(__file__).parent))\nfrom conformance import plugin\nasync def main():\n    endpoint = await plugin().start('127.0.0.1:0')\n    print(endpoint.address, flush=True)\n    await endpoint.server.wait_for_termination()\nasyncio.run(main())\n",
                encoding="utf-8",
            )
            process = await asyncio.create_subprocess_exec(
                str(python), "-I", str(source), cwd=scratch, env=environment, stdout=asyncio.subprocess.PIPE, creationflags=spawn_flags()
            )
            try:
                assert process.stdout is not None
                address = (await asyncio.wait_for(process.stdout.readline(), 30)).decode().strip()
                # Run Every Reference-Host Rule Against the Installed Package.
                status, stdout, stderr = await host.run(address)
                self.assertEqual(status, 0, stdout + stderr)
            finally:
                if process.returncode is None:
                    process.kill()
                await process.wait()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
