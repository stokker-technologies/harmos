# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Composed Library and Product Client Roots"""

# === Imports === #

import json
import unittest
from pathlib import Path

import grpc

from harmos import Client
from tests.support.client_protocol import diagnostics, jobs, progress, status

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ClientRoot(unittest.IsolatedAsyncioTestCase):
    async def test_library_and_product_services_share_parents_and_keep_watch_cardinality(self):
        server = grpc.aio.server()
        # The Native Service Names Are the Dispatch Authority for the Generated Root.
        methods = [
            ("application.v1.Jobs", "Start", jobs.JobsStartRequest, jobs.JobsStartResponse),
        ]
        for service, method, request, response in methods:

            def unary(value, context, response=response):
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

        async def get_status(request, context):
            response = status.StatusGetResponse()
            response.status.identity.application_name = "selection"
            return response

        async def get_diagnostics(request, context):
            return diagnostics.DiagnosticsGetResponse(name=request.name)

        for service, request, response, handler in [
            ("harmos.v1.Status", status.StatusGetRequest, status.StatusGetResponse, get_status),
            ("harmos.v1.status.Diagnostics", diagnostics.DiagnosticsGetRequest, diagnostics.DiagnosticsGetResponse, get_diagnostics),
        ]:
            server.add_generic_rpc_handlers((grpc.method_handlers_generic_handler(service, {
                "Get": grpc.unary_unary_rpc_method_handler(handler, request_deserializer=request.FromString, response_serializer=response.SerializeToString)
            }),))

        async def watch(request, context):
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
                table = json.loads((Path(__file__).resolve().parents[3] / "tests/fixtures/client/resources.json").read_text())
                client = Client(channel, table)
                self.assertEqual((await client.status.get(status.StatusGetRequest())).status.identity.application_name, "selection")
                self.assertEqual((await client.status.diagnostics.get(diagnostics.DiagnosticsGetRequest(name="selection"))).name, "selection")
                self.assertEqual((await client.jobs.start(jobs.JobsStartRequest(value="selection"))).value, "selection")
                self.assertEqual(
                    [item.value async for item in client.jobs.progress.watch(progress.ProgressWatchRequest(value="selection"))],
                    ["opened:selection", "finished:selection"],
                )
                with self.assertRaises(ValueError):
                    Client(channel, table, table)
                with self.assertRaises(ValueError):
                    Client(channel, [{**table[0], "face": "context"}])
        finally:
            await server.stop(None)


# === Entry Point === #

# === Public Functions === #

# === Private Functions === #
