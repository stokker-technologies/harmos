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
import unittest
from collections.abc import AsyncIterator
from unittest.mock import patch

import grpc

from harmos.client_transport import stream
from tests.support.client_protocol import progress

# isort: split
from tests.fixtures.client.client import ClientRoot as Client

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class CallerContext(unittest.IsolatedAsyncioTestCase):
    async def test_one_call_preserves_metadata_deadline_override_and_cancellation(self) -> None:
        # Serve a Watch That Acknowledges Metadata and Waits for Caller Cancellation.
        stopped = asyncio.Event()
        server = grpc.aio.server()

        async def watch(
            request: progress.ProgressWatchRequest, context: grpc.aio.ServicerContext
        ) -> AsyncIterator[progress.ProgressWatchResponse]:
            try:
                token = str(next((value for key, value in (context.invocation_metadata() or ()) if key == "authorization"), ""))
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
                client = Client(native)
                # Supply All Three Controls to One Generated Resource Call.
                with patch("tests.fixtures.client.client.stream", wraps=stream) as dispatch:
                    call = client.jobs.progress.watch(
                        progress.ProgressWatchRequest(value="selection"), metadata=(("authorization", "Bearer fixture"),), timeout=17
                    )
                self.assertEqual(dispatch.call_args.kwargs["timeout"], 17)
                self.assertEqual((await call.read()).value, "Bearer fixture")
                self.assertTrue(call.cancel())
                with self.assertRaises(asyncio.CancelledError):
                    await call.read()
                await stopped.wait()
                # A Following Call Keeps Its Own Default Options.
                stopped.clear()
                with patch("tests.fixtures.client.client.stream", wraps=stream) as dispatch:
                    following = client.jobs.progress.watch(progress.ProgressWatchRequest(value="selection"))
                self.assertIsNone(dispatch.call_args.kwargs["timeout"])
                self.assertIsNone(dispatch.call_args.kwargs["metadata"])
                self.assertEqual((await following.read()).value, "")
                following.cancel()
                await stopped.wait()
        finally:
            await server.stop(None)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
