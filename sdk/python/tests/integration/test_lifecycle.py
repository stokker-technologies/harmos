# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Lifecycle Operation Authoring"""

# === Imports === #

# Standard Library Imports
import asyncio
import io
import os
from unittest import IsolatedAsyncioTestCase
from unittest.mock import patch

# Third Party Imports
import grpc
from google.protobuf.empty_pb2 import Empty
from google.protobuf.timestamp_pb2 import Timestamp
from grpc.aio import EOF

from harmos import VISIBILITY_PUBLIC
from harmos._wire.host.v1.time_pb2 import TimeGetResponse
from harmos._wire.plugin.v1.instance_pb2 import InstanceGetRequest, InstanceStopRequest
from harmos._wire.plugin.v1.instance_pb2_grpc import InstanceStub
from harmos.plugin import Catalog, Context, Endpoint, HostOperation, Plugin, Registrar, operation

# Local Imports
from tests.fixtures.conformance import Value
from tests.fixtures.lifecycle import Author
from tests.support import calls

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Lifecycle(IsolatedAsyncioTestCase):
    async def test_run_announces_only_managed_launches_and_stops_cleanly(self) -> None:
        for configured in (None, "127.0.0.1:0"):
            await self.assert_run_output(configured)

    async def assert_run_output(self, configured: str | None) -> None:
        environment = patch.dict(os.environ)
        environment.start()
        if configured is None:
            os.environ.pop("HARMOS_ADDRESS", None)
        else:
            os.environ["HARMOS_ADDRESS"] = configured
        stdout = io.StringIO()
        stderr = io.StringIO()
        output = patch("sys.stdout", stdout)
        diagnostics = patch("sys.stderr", stderr)
        output.start()
        diagnostics.start()
        endpoint = None
        serving = None
        plugin = Plugin(id="plain", version="1.0.0", register=lambda ctx: ctx.register(Catalog(object())))
        start = plugin.start
        started: asyncio.Future[Endpoint] = asyncio.get_running_loop().create_future()

        async def capture(address: str) -> Endpoint:
            listener = await start(address)
            started.set_result(listener)
            return listener

        binding = patch.object(plugin, "start", side_effect=capture)
        binding.start()
        try:
            serving = asyncio.create_task(plugin.run())
            endpoint = await started
            await endpoint.stop()
            await serving
            expected = f"harmos listening {endpoint.address}\n" if configured is not None else ""
            self.assertEqual(stdout.getvalue(), expected)
            self.assertEqual(stderr.getvalue(), "")
        finally:
            if endpoint is not None:
                await endpoint.stop()
            if serving is not None:
                serving.cancel()
                await asyncio.gather(serving, return_exceptions=True)
            binding.stop()
            diagnostics.stop()
            output.stop()
            environment.stop()

    async def test_hooks_use_empty_operations_and_share_instance_state(self) -> None:
        author = Author("success")
        author.release.set()
        endpoint = await Plugin(
            id="lifecycle", version="1.0.0", register=lambda ctx: ctx.register(Catalog(author, requires=[HostOperation.TIME_GET]))
        ).start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.stop)
        channel = grpc.aio.insecure_channel(endpoint.address)
        self.addAsyncCleanup(channel.close)
        client = InstanceStub(channel)
        description = await client.Get(InstanceGetRequest(revision=1))
        hooks = [operation for operation in description.declarations.operations if operation.id != "lifecycle.state"]
        self.assertEqual({hook.id for hook in hooks}, {"lifecycle.initialize", "lifecycle.finalize"})
        self.assertTrue(all(hook.request == "google.protobuf.Empty" and hook.response == "google.protobuf.Empty" for hook in hooks))
        initialized = await calls.invoke(client, "lifecycle.initialize", Empty())
        child = (await initialized.read()).child
        await calls.reply(initialized, child, TimeGetResponse(now=Timestamp(seconds=1)))
        self.assertEqual((await initialized.read()).response.payload, b"")
        self.assertIs(await initialized.read(), EOF)
        state = await calls.invoke(client, "lifecycle.state", Value())
        self.assertEqual(Value.FromString((await state.read()).response.payload).value, b"initialized")
        self.assertIs(await state.read(), EOF)
        finalized = await calls.invoke(client, "lifecycle.finalize", Empty())
        self.assertEqual((await finalized.read()).response.payload, b"")
        self.assertIs(await finalized.read(), EOF)
        await client.Stop(InstanceStopRequest())
        self.assertEqual(author.finalized, 1)

    async def test_catalog_hooks_share_each_catalogs_instance(self) -> None:
        first = Author("success")
        second = Author("success")

        class Hooks:
            def __init__(self, author: Author) -> None:
                self.author = author

            async def finalize(self, ctx: Context) -> None:
                if ctx.plugin is not self:
                    raise ValueError("Finalize Catalog: Use Its Registered Instance")
                self.author.finalized += 1

        def register(ctx: Registrar) -> None:
            ctx.register(Catalog(Hooks(first)))
            ctx.register(Catalog(Hooks(second)))

        endpoint = await Plugin(id="catalogs", version="1.0.0", register=register).start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.stop)
        channel = grpc.aio.insecure_channel(endpoint.address)
        self.addAsyncCleanup(channel.close)
        client = InstanceStub(channel)
        finalized = await calls.invoke(client, "catalogs.finalize", Empty())
        self.assertEqual((await finalized.read()).response.payload, b"")
        self.assertIs(await finalized.read(), EOF)
        self.assertEqual((first.finalized, second.finalized), (1, 1))

    async def test_ordinary_operations_cannot_claim_lifecycle_names(self) -> None:
        class Invalid:
            @operation("reserved.initialize", request=Empty, response=Empty, visibility=VISIBILITY_PUBLIC)
            async def run(self, ctx: Context, request: Empty) -> Empty:
                return Empty()

        plugin = Plugin(id="reserved", version="1.0.0", register=lambda ctx: ctx.register(Catalog(Invalid())))
        self.assertRaises(ValueError, plugin.declarations)

    async def test_hooks_must_be_async(self) -> None:
        class Invalid:
            def initialize(self, ctx: Context) -> None:
                pass

        plugin = Plugin(id="reserved", version="1.0.0", register=lambda ctx: ctx.register(Catalog(Invalid())))
        self.assertRaises(ValueError, plugin.declarations)

    async def test_plugin_without_hooks_has_no_lifecycle_operations(self) -> None:
        declarations = Plugin(id="plain", version="1.0.0", register=lambda ctx: ctx.register(Catalog(object()))).declarations()
        self.assertEqual(list(declarations.operations), [])


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
