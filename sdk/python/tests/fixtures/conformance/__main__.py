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
import sys
from argparse import ArgumentParser

from harmos.plugin import Catalog, HostOperation, Plugin
from tests.fixtures.conformance import plugin
from tests.fixtures.conformance.fixture import ClockFixture

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
    parser.add_argument("--address", default="127.0.0.1:0")
    parser.add_argument("--clock-only", action="store_true", help="Serve Only the Host Clock Journey")
    parser.add_argument("--shutdown-on-stdin", action="store_true", help="Close When the Process Owner Closes Standard Input")
    options = parser.parse_args()
    selected = (
        Plugin(
            id="conformance",
            version="1.0.0",
            register=lambda ctx: ctx.register(Catalog(ClockFixture(), requires=[HostOperation.TIME_SLEEP, HostOperation.TIME_GET])),
        )
        if options.clock_only
        else plugin()
    )
    if not options.shutdown_on_stdin:
        await selected.run(options.address)
        return
    serving = asyncio.create_task(selected.run(options.address))
    try:
        await asyncio.to_thread(sys.stdin.buffer.read)
    finally:
        serving.cancel()
        try:
            await serving
        except asyncio.CancelledError:
            pass


# === Entry Point === #

if __name__ == "__main__":
    asyncio.run(_main())
