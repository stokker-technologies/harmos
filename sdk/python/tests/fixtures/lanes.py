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

from harmos import VISIBILITY_PUBLIC
from harmos.plugin import Catalog, Context, Lane, Plugin, operation
from tests.fixtures.conformance import Value

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Author:
    def __init__(self) -> None:
        self.release: dict[bytes, asyncio.Event] = {}
        self.entered: list[str] = []

    async def run(self, request: Value) -> Value:
        release = asyncio.Event()
        self.release[request.value] = release
        self.entered.append(request.value.decode())
        print(f"entered:{request.value.decode()}", flush=True)
        await release.wait()
        return request


# === Public Functions === #

# === Private Functions === #


async def _main() -> None:
    id = sys.argv[1]
    plugin_id = sys.argv[2]
    lanes = [Lane("Serial", serial=True), Lane("parallel", concurrent=2), Lane("other", serial=True)]

    class LaneAuthor(Author):
        @operation(f"{id}.first", request=Value, response=Value, visibility=VISIBILITY_PUBLIC, lane=lanes[0])
        async def first(self, _: Context, request: Value) -> Value:
            return await self.run(request)

        @operation(f"{id}.second", request=Value, response=Value, visibility=VISIBILITY_PUBLIC, lane=lanes[0])
        async def second(self, _: Context, request: Value) -> Value:
            return await self.run(request)

        @operation(f"{id}.parallel", request=Value, response=Value, visibility=VISIBILITY_PUBLIC, lane=lanes[1])
        async def parallel(self, _: Context, request: Value) -> Value:
            return await self.run(request)

        @operation(f"{id}.other", request=Value, response=Value, visibility=VISIBILITY_PUBLIC, lane=lanes[2])
        async def other(self, _: Context, request: Value) -> Value:
            return await self.run(request)

        @operation(f"{id}.open", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
        async def open(self, _: Context, request: Value) -> Value:
            return await self.run(request)

    author = LaneAuthor()
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
