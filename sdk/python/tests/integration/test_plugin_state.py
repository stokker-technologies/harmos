# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Durable Plugin State Child Calls"""

# === Imports === #

# Standard Library Imports
from asyncio import Queue, create_task
from unittest import IsolatedAsyncioTestCase

# Third Party Imports

# Local Imports
from harmos._wire.host.v1.plugins.state_pb2 import StateGetResponse, StatePutRequest, StatePutResponse
from harmos._wire.plugin.v1.invocation_pb2 import ChildResponse, Request
from harmos.plugin import Catalog, Context, HostOperation, Plugin, Registrar

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class PluginStateCalls(IsolatedAsyncioTestCase):
    async def test_initialize_context_state_preserves_bytes_and_revision(self) -> None:
        def register(ctx: Registrar) -> None:
            ctx.register(Catalog(object(), requires=(HostOperation.PLUGINS_STATE_GET, HostOperation.PLUGINS_STATE_PUT)))

        plugin = Plugin(id="example", version="1.0.0", register=register)
        declarations = plugin.declarations()
        frames = Queue()
        context = Context(
            Request(operation="example.initialize"),
            "example",
            {requirement.id: requirement.fingerprint for requirement in declarations.requires},
            frames,
        )
        getting = create_task(context.state.get())
        child = (await frames.get()).child
        self.assertEqual(child.operation, "harmos.plugin.v1.plugins.State/Get")
        await context._reply(ChildResponse(call=child.call, payload=StateGetResponse(value=b"old", revision=3).SerializeToString()))
        saved = await getting
        self.assertEqual((saved.value, saved.revision), (b"old", 3))
        putting = create_task(context.state.put(saved.revision, b"intent"))
        child = (await frames.get()).child
        request = StatePutRequest.FromString(child.payload)
        self.assertEqual((request.expected_revision, request.value), (3, b"intent"))
        await context._reply(ChildResponse(call=child.call, payload=StatePutResponse(revision=4).SerializeToString()))
        self.assertEqual(await putting, 4)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
