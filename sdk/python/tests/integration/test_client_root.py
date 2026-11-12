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
from importlib import import_module
import sys
import unittest
from pathlib import Path

import grpc

from harmos import Client

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures/client"
sys.path.insert(0, str(FIXTURES))
plugins = import_module("library.v1.plugins_pb2")
settings = import_module("library.v1.plugins.settings_pb2")
jobs = import_module("application.v1.jobs_pb2")
progress = import_module("application.v1.jobs.progress_pb2")

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
            ("library.v1.Plugins", "Get", plugins.PluginsGetRequest, plugins.PluginsGetResponse),
            ("library.v1.plugins.Settings", "Get", settings.SettingsGetRequest, settings.SettingsGetResponse),
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
                table = json.loads((Path(__file__).resolve().parents[3] / "fixtures/client/resources.json").read_text())
                client = Client(channel, table)
                self.assertEqual((await client.plugins.get(plugins.PluginsGetRequest(value="selection"))).value, "selection")
                self.assertEqual((await client.plugins.settings.get(settings.SettingsGetRequest(value="selection"))).value, "selection")
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
