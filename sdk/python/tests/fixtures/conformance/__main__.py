# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Serve the Fixed Conformance Declarations at an Explicit Remote Address."""

# === Imports === #

import asyncio
import importlib
import sys
from argparse import ArgumentParser
from pathlib import Path

from harmos.plugin import HostOperation, Plugin

if not __package__:
    sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
    __package__ = "tests.fixtures.conformance"

conformance_plugin = importlib.import_module(__package__).plugin
ClockFixture = importlib.import_module(__package__ + ".fixture").ClockFixture

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #

# === Private Functions === #


async def _main() -> None:
    if sys.argv[1:2] == ["--install-itself"]:
        from .install_itself import run

        await run(sys.argv[2:])
        return
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--address", required=True)
    parser.add_argument("--clock-only", action="store_true", help="Serve Only the Host Clock Journey")
    parser.add_argument("--shutdown-on-stdin", action="store_true", help="Close When the Process Owner Closes Standard Input")
    options = parser.parse_args()
    selected = (
        Plugin(ClockFixture(), id="conformance", version="1.0.0", requires=[HostOperation.TIME_SLEEP, HostOperation.TIME_GET])
        if options.clock_only
        else conformance_plugin()
    )
    endpoint = await selected.start(options.address)
    print(f"harmos listening {endpoint.address}", flush=True)
    try:
        if options.shutdown_on_stdin:
            await asyncio.to_thread(sys.stdin.buffer.read)
        else:
            await endpoint.server.wait_for_termination()
    finally:
        await endpoint.close()


# === Entry Point === #

if __name__ == "__main__":
    asyncio.run(_main())
