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
from tests.support import bootstrap, calls

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Lifecycle(IsolatedAsyncioTestCase):
    async def test_managed_run_announces_once_and_writes_nothing(self) -> None:
        launches = await bootstrap.start()
        self.addAsyncCleanup(launches.stop)
        stdout, stderr = io.StringIO(), io.StringIO()
        plugin = Plugin(id="plain", version="1.0.0", register=lambda ctx: ctx.register(Catalog(object())))
        with patch.dict(os.environ, launches.environment({})), patch("sys.stdout", stdout), patch("sys.stderr", stderr):
            serving = asyncio.create_task(plugin.run())
            announce = await launches.announces.get()
            self.assertEqual(announce.token, bootstrap.TOKEN)
            host, port = announce.address.rsplit(":", 1)
            self.assertEqual((host, port != "0"), ("127.0.0.1", True))
            channel = grpc.aio.insecure_channel(announce.address)
            self.addAsyncCleanup(channel.close)
            self.assertEqual((await InstanceStub(channel).Get(InstanceGetRequest(revision=1))).id, "plain")
            serving.cancel()
            await asyncio.gather(serving, return_exceptions=True)
        self.assertEqual((stdout.getvalue(), stderr.getvalue()), ("", ""))
        self.assertTrue(launches.announces.empty())

    async def test_refused_announce_fails_the_start_with_the_host_message(self) -> None:
        launches = await bootstrap.start(refusal="Launch Credential Is Invalid")
        self.addAsyncCleanup(launches.stop)
        plugin = Plugin(id="plain", version="1.0.0", register=lambda ctx: ctx.register(Catalog(object())))
        with patch.dict(os.environ, launches.environment({})):
            with self.assertRaisesRegex(RuntimeError, "Launch Credential Is Invalid"):
                await plugin.run()

    async def test_one_launch_variable_is_a_start_error_naming_the_other(self) -> None:
        plugin = Plugin(id="plain", version="1.0.0", register=lambda ctx: ctx.register(Catalog(object())))
        for present, missing in (("HARMOS_HOST_ENDPOINT", "HARMOS_LAUNCH_TOKEN"), ("HARMOS_LAUNCH_TOKEN", "HARMOS_HOST_ENDPOINT")):
            with patch.dict(os.environ, {present: "127.0.0.1:9"}):
                os.environ.pop(missing, None)
                with self.assertRaises(ValueError):
                    await plugin.run()

    async def test_run_without_launch_variables_serves_silently(self) -> None:
        stdout, stderr = io.StringIO(), io.StringIO()
        plugin = Plugin(id="plain", version="1.0.0", register=lambda ctx: ctx.register(Catalog(object())))
        start = plugin.start
        started: asyncio.Future[Endpoint] = asyncio.get_running_loop().create_future()

        async def capture(address: str) -> Endpoint:
            listener = await start(address)
            started.set_result(listener)
            return listener

        with (
            patch.dict(os.environ),
            patch.object(plugin, "start", side_effect=capture),
            patch("sys.stdout", stdout),
            patch("sys.stderr", stderr),
        ):
            os.environ.pop("HARMOS_HOST_ENDPOINT", None)
            os.environ.pop("HARMOS_LAUNCH_TOKEN", None)
            serving = asyncio.create_task(plugin.run())
            endpoint = await started
            self.assertNotEqual(endpoint.address.rsplit(":", 1)[1], "0")
            await endpoint.stop()
            await serving
        self.assertEqual((stdout.getvalue(), stderr.getvalue()), ("", ""))

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

        self.assertRaises(
            ValueError, lambda: Plugin(id="reserved", version="1.0.0", register=lambda ctx: ctx.register(Catalog(Invalid()))).declarations()
        )

    async def test_hooks_must_be_async(self) -> None:
        class Invalid:
            def initialize(self, ctx: Context) -> None:
                pass

        self.assertRaises(
            ValueError, lambda: Plugin(id="reserved", version="1.0.0", register=lambda ctx: ctx.register(Catalog(Invalid()))).declarations()
        )

    async def test_plugin_without_hooks_has_no_lifecycle_operations(self) -> None:
        declarations = Plugin(id="plain", version="1.0.0", register=lambda ctx: ctx.register(Catalog(object()))).declarations()
        self.assertEqual(list(declarations.operations), [])


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
