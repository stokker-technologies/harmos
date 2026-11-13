# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Composed Library and Product Client Roots"""

# === Imports === #

import unittest
from collections.abc import AsyncIterator
from typing import assert_type

import grpc

from harmos import Client as LibraryClient
from tests.support.client_protocol import diagnostics, jobs, progress, status

# isort: split
from tests.fixtures.client.client import ClientRoot as Client

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ApplicationClient(LibraryClient):
    def __init__(self, channel: grpc.aio.Channel) -> None:
        super().__init__(channel)
        self.jobs = Client(channel).jobs


class ClientRoot(unittest.IsolatedAsyncioTestCase):
    async def test_library_and_product_services_share_parents_and_keep_watch_cardinality(self) -> None:
        server = grpc.aio.server()
        # The Native Service Names Are the Dispatch Authority for the Generated Root.
        methods = [
            ("application.v1.Jobs", "Start", jobs.JobsStartRequest, jobs.JobsStartResponse),
        ]
        for service, method, request, response in methods:

            def unary(
                value: jobs.JobsStartRequest, context: grpc.aio.ServicerContext, response: type[jobs.JobsStartResponse] = response
            ) -> jobs.JobsStartResponse:
                return response(value=value.value)

            server.add_generic_rpc_handlers(
                (
                    grpc.method_handlers_generic_handler(
                        service,
                        {
                            method: grpc.unary_unary_rpc_method_handler(
                                unary, request_deserializer=request.FromString, response_serializer=response.SerializeToString
                            )
                        },
                    ),
                )
            )

        async def get_status(request: status.StatusGetRequest, context: grpc.aio.ServicerContext) -> status.StatusGetResponse:
            response = status.StatusGetResponse()
            response.status.identity.application_name = "selection"
            return response

        async def get_diagnostics(
            request: diagnostics.DiagnosticsGetRequest, context: grpc.aio.ServicerContext
        ) -> diagnostics.DiagnosticsGetResponse:
            return diagnostics.DiagnosticsGetResponse(name=request.name)

        for service, request, response, handler in [
            ("harmos.v1.Status", status.StatusGetRequest, status.StatusGetResponse, get_status),
            ("harmos.v1.status.Diagnostics", diagnostics.DiagnosticsGetRequest, diagnostics.DiagnosticsGetResponse, get_diagnostics),
        ]:
            server.add_generic_rpc_handlers(
                (
                    grpc.method_handlers_generic_handler(
                        service,
                        {
                            "Get": grpc.unary_unary_rpc_method_handler(
                                handler, request_deserializer=request.FromString, response_serializer=response.SerializeToString
                            )
                        },
                    ),
                )
            )

        async def watch(
            request: progress.ProgressWatchRequest, context: grpc.aio.ServicerContext
        ) -> AsyncIterator[progress.ProgressWatchResponse]:
            yield progress.ProgressWatchResponse(value="opened:" + request.value)
            yield progress.ProgressWatchResponse(value="finished:" + request.value)

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
            async with grpc.aio.insecure_channel(f"127.0.0.1:{port}") as channel:
                client = Client(channel)
                product = ApplicationClient.connect(f"127.0.0.1:{port}")
                self.addAsyncCleanup(product.close)
                assert_type(product, ApplicationClient)
                reply = await product.jobs.start(jobs.JobsStartRequest(value="product"))
                assert_type(reply, jobs.JobsStartResponse)
                self.assertEqual(reply.value, "product")
                self.assertEqual((await client.status.get(status.StatusGetRequest())).status.identity.application_name, "selection")
                self.assertEqual(
                    (await client.status.diagnostics.get(diagnostics.DiagnosticsGetRequest(name="selection"))).name, "selection"
                )
                self.assertEqual((await client.jobs.start(jobs.JobsStartRequest(value="selection"))).value, "selection")
                self.assertEqual(
                    [item.value async for item in client.jobs.progress.watch(progress.ProgressWatchRequest(value="selection"))],
                    ["opened:selection", "finished:selection"],
                )
        finally:
            await server.stop(None)


# === Entry Point === #

# === Public Functions === #

# === Private Functions === #
