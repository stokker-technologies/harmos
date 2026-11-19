# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Lifecycle Story Plugin"""

# === Imports === #

# Standard Library Imports
import asyncio
import sys

# Third Party Imports
from harmos import CLASSIFICATION_PERMANENT, VISIBILITY_PUBLIC, model
from harmos._invocation import FailureError
from harmos.plugin import Catalog, Context, HostOperation, Plugin, operation

# Local Imports
from tests.fixtures.conformance import Value

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Author:
    def __init__(self, mode: str) -> None:
        self.mode = mode
        self.release = asyncio.Event()
        self.finalized = 0
        self.active = 0
        self.value = b""

    async def initialize(self, ctx: Context) -> None:
        self.active += 1
        print("initialize", flush=True)
        try:
            await self.release.wait()
            if self.mode == "failure":
                raise FailureError(model.Failure(classification=CLASSIFICATION_PERMANENT, code="acquire_failed", message="Acquire Failed"))
            await ctx.host.time.get()
            self.value = b"initialized"
        finally:
            self.active -= 1
            print("initialized", flush=True)

    async def finalize(self, ctx: Context) -> None:
        if ctx.plugin is not self:
            raise ValueError("Finalize Plugin: Use the Shared Instance")
        self.finalized += 1
        print(f"finalize:{self.active}:{self.finalized}", flush=True)
        if self.active:
            raise ValueError("Finalize Plugin: Settle Active Operations First")
        if self.mode == "finalize_failure":
            raise FailureError(model.Failure(classification=CLASSIFICATION_PERMANENT, code="release_failed", message="Release Failed"))

    @operation("lifecycle.state", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def state(self, ctx: Context, request: Value) -> Value:
        if ctx.plugin is not self:
            raise ValueError("Read Plugin State: Use the Shared Instance")
        self.active += 1
        print("operation", flush=True)
        try:
            if request.value == b"hold":
                await asyncio.Event().wait()
            return Value(value=self.value)
        finally:
            self.active -= 1
            print("settled", flush=True)


# === Public Functions === #

# === Private Functions === #


async def _main() -> None:
    author = Author(sys.argv[1])
    endpoint = await Plugin(id="lifecycle", version="1.0.0", register=lambda ctx: ctx.register(Catalog(author, requires=[HostOperation.TIME_GET]))).start("127.0.0.1:0")
    print(f"listening:{endpoint.address}", flush=True)
    try:
        while True:
            command = await asyncio.to_thread(sys.stdin.readline)
            if not command or command.strip() == "quit":
                return
            if command.strip() == "release":
                author.release.set()
            if command.strip() == "state":
                print(f"state:{author.active}:{author.finalized}", flush=True)
    finally:
        await endpoint.stop()


# === Entry Point === #

if __name__ == "__main__":
    asyncio.run(_main())
