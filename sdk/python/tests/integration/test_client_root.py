# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Composed Library and Product Client Roots"""

# === Imports === #

from collections.abc import AsyncIterator
from typing import assert_type

import grpc

# isort: split
from unittest import IsolatedAsyncioTestCase

from grpc.aio import ServicerContext

from harmos import SettingValues
from harmos._wire.client.v1.plugins.settings_pb2 import SettingsSetRequest, SettingsSetResponse
from harmos._wire.client.v1.status.diagnostics_pb2 import DiagnosticsGetRequest, DiagnosticsGetResponse
from harmos._wire.client.v1.status_pb2 import StatusGetRequest, StatusGetResponse
from tests.fixtures.client import Client
from tests.fixtures.client._wire.application.v1.jobs.progress_pb2 import ProgressWatchRequest, ProgressWatchResponse
from tests.fixtures.client._wire.application.v1.jobs_pb2 import JobsStartRequest, JobsStartResponse
from tests.fixtures.client._wire.application.v1.plugins.annotations_pb2 import AnnotationsGetRequest, AnnotationsGetResponse

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ApplicationClient(Client):
    pass


class ClientRoot(IsolatedAsyncioTestCase):
    async def test_library_and_product_services_share_parents_and_keep_watch_cardinality(self) -> None:
        server = grpc.aio.server()
        # The Native Service Names Are the Dispatch Authority for the Generated Root.
        methods = [
            ("application.v1.Jobs", "Start", JobsStartRequest, JobsStartResponse),
        ]
        for service, method, request, response in methods:

            def unary(value: JobsStartRequest, context: ServicerContext, response: type[JobsStartResponse] = response) -> JobsStartResponse:
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

        async def get_status(request: StatusGetRequest, context: ServicerContext) -> StatusGetResponse:
            response = StatusGetResponse()
            response.status.identity.application_name = "selection"
            return response

        async def get_annotation(request: AnnotationsGetRequest, context: ServicerContext) -> AnnotationsGetResponse:
            return AnnotationsGetResponse(value=request.name)

        server.add_generic_rpc_handlers(
            (
                grpc.method_handlers_generic_handler(
                    "application.v1.plugins.Annotations",
                    {
                        "Get": grpc.unary_unary_rpc_method_handler(
                            get_annotation,
                            request_deserializer=AnnotationsGetRequest.FromString,
                            response_serializer=AnnotationsGetResponse.SerializeToString,
                        )
                    },
                ),
            )
        )

        async def get_diagnostics(request: DiagnosticsGetRequest, context: ServicerContext) -> DiagnosticsGetResponse:
            return DiagnosticsGetResponse(name=request.name)

        for service, request, response, handler in [
            ("harmos.client.v1.Status", StatusGetRequest, StatusGetResponse, get_status),
            ("harmos.client.v1.status.Diagnostics", DiagnosticsGetRequest, DiagnosticsGetResponse, get_diagnostics),
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

        async def watch(request: ProgressWatchRequest, context: ServicerContext) -> AsyncIterator[ProgressWatchResponse]:
            yield ProgressWatchResponse(value="opened:" + request.value)
            yield ProgressWatchResponse(value="finished:" + request.value)

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

        async def set_settings(request: SettingsSetRequest, context: ServicerContext) -> SettingsSetResponse:
            return SettingsSetResponse(revision=request.request_id.encode())

        server.add_generic_rpc_handlers(
            (
                grpc.method_handlers_generic_handler(
                    "harmos.client.v1.plugins.Settings",
                    {
                        "Set": grpc.unary_unary_rpc_method_handler(
                            set_settings,
                            request_deserializer=SettingsSetRequest.FromString,
                            response_serializer=SettingsSetResponse.SerializeToString,
                        )
                    },
                ),
            )
        )
        port = server.add_insecure_port("127.0.0.1:0")
        await server.start()
        try:
            channel = grpc.aio.insecure_channel(f"127.0.0.1:{port}")
            self.addAsyncCleanup(channel.close)
            client = Client(channel)
            product = ApplicationClient.connect(f"127.0.0.1:{port}")
            self.addAsyncCleanup(product.close)
            assert_type(product, ApplicationClient)
            reply = await product.jobs.start("product")
            assert_type(reply, str)
            self.assertEqual(reply, "product")
            self.assertEqual((await client.status.get()).status.identity.application_name, "selection")
            self.assertEqual((await client.status.diagnostics.get("selection", 0, 0)).name, "selection")
            self.assertEqual((await client.jobs.start("selection")), "selection")
            self.assertEqual((await client.plugins.annotations.get("example")), "example")
            self.assertEqual(
                [item async for item in client.jobs.progress.watch("selection")],
                ["opened:selection", "finished:selection"],
            )
            generated = await client.plugins.settings.set("example", SettingValues(), b"")
            self.assertTrue(generated.revision)
            replayed = await client.plugins.settings.set("example", SettingValues(), b"", request_id="replay")
            self.assertEqual(replayed.revision, b"replay")
        finally:
            await server.stop(None)


# === Entry Point === #

# === Public Functions === #

# === Private Functions === #
