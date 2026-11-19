# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Plugin Lane Declarations and Admission Fixtures."""

# === Imports === #

import asyncio
import sys
from collections.abc import Callable

from harmos import VISIBILITY_PUBLIC
from harmos.plugin import Catalog, Context, Lane, Plugin, operation
from tests.fixtures.conformance import Value

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Author:
    def __init__(self, id: str, lanes: list[Lane]) -> None:
        self.release: dict[bytes, asyncio.Event] = {}
        self.entered: list[str] = []
        for name, lane in (("first", lanes[0]), ("second", lanes[0]), ("parallel", lanes[1]), ("other", lanes[2]), ("open", None)):
            self.__dict__[name] = self.handler(f"{id}.{name}", lane)

    def handler(self, id: str, lane: Lane | None) -> Callable:
        @operation(id, request=Value, response=Value, visibility=VISIBILITY_PUBLIC, lane=lane)
        async def run(_: Context, request: Value) -> Value:
            release = asyncio.Event()
            self.release[request.value] = release
            self.entered.append(request.value.decode())
            print(f"entered:{request.value.decode()}", flush=True)
            await release.wait()
            return request

        return run


# === Public Functions === #

# === Private Functions === #


async def _main() -> None:
    id = sys.argv[1]
    plugin_id = sys.argv[2]
    lanes = [Lane("instrument", serial=True), Lane("parallel", concurrent=2), Lane("other", serial=True)]
    author = Author(id, lanes)
    endpoint = await Plugin(id=plugin_id, version="1.0.0", register=lambda ctx: ctx.register(Catalog(author)), lanes=lanes).start("127.0.0.1:0")
    print(f"listening:{endpoint.address}", flush=True)
    try:
        while True:
            command = (await asyncio.to_thread(sys.stdin.readline)).strip()
            if command == "quit":
                return
            if command == "state":
                print("state:" + ",".join(author.entered), flush=True)
                continue
            author.release[command.encode()].set()
    finally:
        await endpoint.close()


# === Entry Point === #

if __name__ == "__main__":
    asyncio.run(_main())
