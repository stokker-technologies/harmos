# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Composed Library and Product Client Roots"""

# === Imports === #

import shutil
import subprocess
import tempfile
from collections.abc import AsyncIterator, Callable
from importlib.util import module_from_spec, spec_from_file_location
from inspect import signature
from pathlib import Path
from typing import Protocol, assert_type, cast

import grpc

# isort: split
from unittest import IsolatedAsyncioTestCase

from google.protobuf.descriptor_pb2 import FileDescriptorProto, FileDescriptorSet
from grpc.aio import Channel, ServicerContext
from harmos_tools.process import spawn_flags
from harmos_tools.source.protocol import narrow_callers

from harmos import SettingValues
from harmos._generated.host import HostRoot
from harmos._wire.host.v1.operations_pb2 import OperationsListRequest, OperationsListResponse
from harmos._wire.host.v1.plugins.settings_pb2 import SettingsSetRequest, SettingsSetResponse
from harmos._wire.host.v1.status.diagnostics_pb2 import DiagnosticsGetRequest, DiagnosticsGetResponse
from harmos._wire.host.v1.status_pb2 import StatusGetRequest, StatusGetResponse
from harmos._wire.plugin.v1.invocation_pb2 import Request
from harmos.client import OperationsResource
from harmos.plugin import Context
from tests.fixtures.client import Client
from tests.fixtures.client._wire.application.host.v1.jobs.progress_pb2 import ProgressWatchRequest, ProgressWatchResponse
from tests.fixtures.client._wire.application.host.v1.jobs_pb2 import JobsStartRequest, JobsStartResponse
from tests.fixtures.client._wire.application.host.v1.plugins.annotations_pb2 import AnnotationsGetRequest, AnnotationsGetResponse

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ApplicationClient(Client):
    pass


class NarrowedClient(Protocol):
    operations: OperationsResource
    time: object


class ClientRoot(IsolatedAsyncioTestCase):
    async def test_composition_narrows_callers_without_inheriting_excluded_methods(self) -> None:
        # Narrow Shared Calls in Each Direction and Generate Both Audience Surfaces.
        root = Path(__file__).resolve().parents[4]
        descriptors = FileDescriptorSet.FromString((root / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
        descriptors.file.append(FileDescriptorProto.FromString(JobsStartRequest.DESCRIPTOR.file.serialized_pb))
        narrow_callers(descriptors, {"harmos.host.v1.Time/Get": "PLUGINS", "harmos.host.v1.Time/Sleep": "CLIENTS"})
        scratch = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, scratch)
        client_file = scratch / "client.py"
        host_file = scratch / "host.py"
        descriptor_file = scratch / "descriptors.bin"
        descriptor_file.write_bytes(descriptors.SerializeToString())
        program = "\n".join(
            [
                "import sys",
                "from pathlib import Path",
                "from google.protobuf.descriptor_pb2 import FileDescriptorSet",
                "from harmos_tools.build.resource_roots import python, python_host, table",
                "schema = FileDescriptorSet.FromString(Path(sys.argv[1]).read_bytes())",
                "methods = table(schema)",
                "clients = [method for method in methods if method['face'] == 'client']",
                "hosts = [method for method in methods if method['face'] == 'context']",
                "python(Path(sys.argv[2]), clients, schema, module_prefixes={'application': 'tests.fixtures.client._wire.application'})",
                "python_host(Path(sys.argv[3]), hosts, schema)",
            ]
        )
        subprocess.run(
            [
                "uv",
                "run",
                "--locked",
                "--no-sync",
                "--project",
                str(root / "tools/python"),
                "python",
                "-c",
                program,
                str(descriptor_file),
                str(client_file),
                str(host_file),
            ],
            check=True,
            creationflags=spawn_flags(),
        )
        client_spec = spec_from_file_location("narrowed_client", client_file)
        host_spec = spec_from_file_location("narrowed_host", host_file)
        assert client_spec is not None and client_spec.loader is not None
        assert host_spec is not None and host_spec.loader is not None
        client_module = module_from_spec(client_spec)
        host_module = module_from_spec(host_spec)
        client_spec.loader.exec_module(client_module)
        host_spec.loader.exec_module(host_module)
        client_factory = cast(Callable[[Channel], NarrowedClient], client_module.ClientRoot)
        host_factory = cast(Callable[[Context], HostRoot], host_module.HostRoot)

        # Serve an Allowed Library Call Through the Generated Connection Root.
        server = grpc.aio.server()

        async def discover(request: OperationsListRequest, context: ServicerContext) -> OperationsListResponse:
            return OperationsListResponse()

        server.add_generic_rpc_handlers(
            (
                grpc.method_handlers_generic_handler(
                    "harmos.host.v1.Operations",
                    {
                        "List": grpc.unary_unary_rpc_method_handler(
                            discover,
                            request_deserializer=OperationsListRequest.FromString,
                            response_serializer=OperationsListResponse.SerializeToString,
                        )
                    },
                ),
            )
        )
        port = server.add_insecure_port("127.0.0.1:0")
        await server.start()
        self.addAsyncCleanup(server.stop, None)
        channel = grpc.aio.insecure_channel(f"127.0.0.1:{port}")
        self.addAsyncCleanup(channel.close)
        client = client_factory(channel)
        self.assertTrue(hasattr(client, "connect"))
        self.assertEqual(await client.operations.list(), [])
        self.assertFalse(hasattr(client.time, "get"))
        self.assertTrue(hasattr(client.time, "sleep"))
        with self.assertRaises(TypeError):
            signature(client_module.PluginsSettingsResource.list).bind(object())

        # The Plugin Keeps Its Shared Call, but Cannot Create Invocations or Set Settings.
        context = Context(Request(), "fixture", {})
        host = host_factory(context)
        self.assertTrue(hasattr(host.time, "get"))
        self.assertFalse(hasattr(host.time, "sleep"))
        self.assertFalse(hasattr(host.invocations, "create"))
        self.assertTrue(hasattr(host.plugins.settings, "list"))
        self.assertFalse(hasattr(host.plugins.settings, "set"))
        signature(host.plugins.settings.list).bind()

        # An Exclusive Declaration Cannot Be Widened at Assembly.
        with self.assertRaises(ValueError):
            narrow_callers(descriptors, {"harmos.host.v1.Invocations/Create": "ALL"})

    async def test_library_and_product_services_share_parents_and_keep_watch_cardinality(self) -> None:
        server = grpc.aio.server()
        # The Native Service Names Are the Dispatch Authority for the Generated Root.
        methods = [
            ("application.host.v1.Jobs", "Start", JobsStartRequest, JobsStartResponse),
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
                    "application.host.v1.plugins.Annotations",
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
            ("harmos.host.v1.Status", StatusGetRequest, StatusGetResponse, get_status),
            ("harmos.host.v1.status.Diagnostics", DiagnosticsGetRequest, DiagnosticsGetResponse, get_diagnostics),
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
                    "application.host.v1.jobs.Progress",
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
                    "harmos.host.v1.plugins.Settings",
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
