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
from pathlib import Path

# Third Party Imports
from harmos import CLASSIFICATION_PERMANENT, VISIBILITY_PUBLIC, SettingDeclaration, SettingValue, model
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
            if self.mode == "settings":
                settings = await ctx.host.plugins.settings.list()
                if settings.value.values["count"].integer != 3:
                    raise ValueError("Initialize Plugin: Restore the Resolved Settings")
            self.value = b"initialized"
        finally:
            self.active -= 1
            print("initialized", flush=True)

    async def finalize(self, ctx: Context) -> None:
        if ctx.plugin is not self:
            raise ValueError("Finalize Plugin: Use the Shared Instance")
        if self.mode == "settings":
            settings = await ctx.host.plugins.settings.list()
            if settings.value.values["count"].integer != 3:
                raise ValueError("Finalize Plugin: Retain the Resolved Settings")
        self.finalized += 1
        print(f"finalize:{self.active}:{self.finalized}", flush=True)
        if self.mode == "managed":
            Path(sys.argv[2]).write_text(str(self.finalized), encoding="utf-8")
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
    requires = [HostOperation.TIME_GET]
    if author.mode == "settings":
        requires.append(HostOperation.PLUGINS_SETTINGS_LIST)
    plugin = Plugin(
        id="lifecycle",
        version="1.0.0",
        register=lambda ctx: ctx.register(
            Catalog(author, requires=requires, settings=[SettingDeclaration(name="count", title="Count", default=SettingValue(integer=3))])
        ),
    )
    if author.mode == "managed":
        author.release.set()
        await plugin.run()
        return
    endpoint = await plugin.start("127.0.0.1:0")
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
