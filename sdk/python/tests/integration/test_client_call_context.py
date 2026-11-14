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
from collections.abc import AsyncIterator
from unittest.mock import patch

import grpc

from harmos._client_transport import stream

# isort: split
from asyncio import Event, Future
from unittest import IsolatedAsyncioTestCase

from grpc.aio import ServicerContext

from tests.fixtures.client import Client
from tests.fixtures.client._wire.application.v1.jobs.progress_pb2 import ProgressWatchRequest, ProgressWatchResponse

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class CallerContext(IsolatedAsyncioTestCase):
    async def test_one_call_preserves_metadata_deadline_override_and_cancellation(self) -> None:
        # Serve a Watch That Acknowledges Metadata and Waits for Caller Cancellation.
        stopped = Event()
        server = grpc.aio.server()

        async def watch(request: ProgressWatchRequest, context: ServicerContext) -> AsyncIterator[ProgressWatchResponse]:
            try:
                token = str(next((value for key, value in (context.invocation_metadata() or ()) if key == "authorization"), ""))
                yield ProgressWatchResponse(value=token)
                await Future()
            finally:
                stopped.set()

        server.add_generic_rpc_handlers(
            (
                grpc.method_handlers_generic_handler(
                    "application.v1.jobs.Progress",
                    {
                        "Watch": grpc.unary_stream_rpc_method_handler(
                            watch,
                            request_deserializer=ProgressWatchRequest.FromString,
                            response_serializer=ProgressWatchResponse.SerializeToString,
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
                with patch("tests.fixtures.client._generated.client.stream", wraps=stream) as dispatch:
                    call = client.jobs.progress.watch("selection", metadata=(("authorization", "Bearer fixture"),), timeout=17)
                    self.assertEqual(await anext(call), "Bearer fixture")
                    self.assertEqual(dispatch.call_args.kwargs["timeout"], 17)
                await call.aclose()
                await stopped.wait()
                # A Following Call Keeps Its Own Default Options.
                stopped.clear()
                with patch("tests.fixtures.client._generated.client.stream", wraps=stream) as dispatch:
                    following = client.jobs.progress.watch("selection")
                    self.assertEqual(await anext(following), "")
                    self.assertIsNone(dispatch.call_args.kwargs["timeout"])
                    self.assertIsNone(dispatch.call_args.kwargs["metadata"])
                await following.aclose()
                await stopped.wait()
        finally:
            await server.stop(None)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
