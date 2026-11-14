# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Run the Built Reference Host Without Owning the Plugin Endpoint."""

# === Imports === #

# Standard Library Imports
import asyncio
import os
from asyncio.subprocess import PIPE
from pathlib import Path

# Third Party Imports
# Local Imports
from harmos_tools.build.paths import output
from harmos_tools.process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[4]

# === Types === #

# === Classes === #

# === Public Functions === #


async def run(address: str, *, binary: Path | None = None) -> tuple[int, str, str]:
    name = "harmos-conformance.exe" if os.name == "nt" else "harmos-conformance"
    profile = os.environ.get("TEST_PROFILE", "dev")
    exported = output(ROOT, "build", profile) / "bin" / name
    local = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug" / name
    binary = binary if binary is not None else Path(os.environ.get("HARMOS_CONFORMANCE_HOST", local if local.is_file() else exported))
    if not binary.is_file():
        raise FileNotFoundError("Build the Reference Host: mise run build -- --package harmos-conformance")
    process = await asyncio.create_subprocess_exec(
        str(binary),
        "--address",
        address,
        stdout=PIPE,
        stderr=PIPE,
        creationflags=spawn_flags(),
    )
    try:
        # This Bounds Transport Hangs Only; It Does Not Advance Scenario Time.
        stdout, stderr = await asyncio.wait_for(process.communicate(), timeout=90)
        status = await process.wait()
        return status, stdout.decode(), stderr.decode()
    finally:
        if process.returncode is None:
            process.kill()
            await process.wait()


# === Private Functions === #

# === Entry Point === #
