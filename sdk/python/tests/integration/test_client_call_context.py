# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Caller Metadata, Deadline Overrides and Cancellation Reach Generated Roots"""

# === Imports === #

# Standard Library Imports
import asyncio
import json
import unittest
from pathlib import Path

import grpc
from harmos import Client
from tests.support.client_protocol import progress

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class CallerContext(unittest.IsolatedAsyncioTestCase):
    async def test_one_call_preserves_metadata_deadline_override_and_cancellation(self):
        # Serve a Watch That Acknowledges Metadata and Waits for Caller Cancellation.
        stopped = asyncio.Event()
        server = grpc.aio.server()

        async def watch(request, context):
            try:
                token = dict(context.invocation_metadata()).get("authorization", "")
                yield progress.ProgressWatchResponse(value=token)
                await asyncio.Future()
            finally:
                stopped.set()

        server.add_generic_rpc_handlers(
            (
                grpc.method_handlers_generic_handler(
                    "application.v1.jobs.Progress",
                    {
                        "Watch": grpc.unary_stream_rpc_method_handler(
                            watch,
                            request_deserializer=progress.ProgressWatchRequest.FromString,
                            response_serializer=progress.ProgressWatchResponse.SerializeToString,
                        )
                    },
                ),
            )
        )
        port = server.add_insecure_port("127.0.0.1:0")
        await server.start()
        try:
            async with grpc.aio.insecure_channel(f"127.0.0.1:{port}") as native:
                # Record the Deadline Option at the Transport Boundary Without Sampling Wall Time.
                options = []

                class Channel:
                    def unary_unary(self, *args, **kwargs):
                        return native.unary_unary(*args, **kwargs)

                    def unary_stream(self, *args, **kwargs):
                        call = native.unary_stream(*args, **kwargs)

                        def start(request, **kwargs):
                            options.append(kwargs)
                            return call(request, **kwargs)

                        return start

                table = json.loads((Path(__file__).resolve().parents[3] / "tests/fixtures/client/resources.json").read_text())
                client = Client(Channel(), table)
                # Supply All Three Controls to One Generated Resource Call.
                call = client.jobs.progress.watch(
                    progress.ProgressWatchRequest(value="selection"), metadata=(("authorization", "Bearer fixture"),), timeout=17
                )
                self.assertEqual((await call.read()).value, "Bearer fixture")
                self.assertEqual(options[0]["timeout"], 17)
                self.assertTrue(call.cancel())
                with self.assertRaises(asyncio.CancelledError):
                    await call.read()
                await stopped.wait()
                # A Following Call Keeps Its Own Default Options.
                stopped.clear()
                following = client.jobs.progress.watch(progress.ProgressWatchRequest(value="selection"))
                self.assertEqual((await following.read()).value, "")
                self.assertEqual(options[1], {})
                following.cancel()
                await stopped.wait()
        finally:
            await server.stop(None)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
