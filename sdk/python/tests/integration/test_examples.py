# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Run the Published Examples Through Real gRPC Connections."""

# === Imports === #

import asyncio
import sys
from asyncio import Event
from asyncio.subprocess import PIPE
from collections.abc import AsyncIterator
from pathlib import Path
from unittest import IsolatedAsyncioTestCase

import grpc
from google.protobuf.wrappers_pb2 import BytesValue
from grpc.aio import EOF, ServicerContext
from harmos_tools.process import spawn_flags

from examples.client import run
from examples.invocation import run as invoke
from harmos import DiscoveredContract, DiscoveredOperation, Position, SettingsDeclarations, SettingValue, SettingValues
from harmos._wire.host.v1.invocations_pb2 import InvocationsCreateRequest, InvocationsCreateResponse
from harmos._wire.host.v1.operations_pb2 import OperationsGetRequest, OperationsGetResponse, OperationsListRequest, OperationsListResponse
from harmos._wire.host.v1.plugins.settings_pb2 import SettingsListRequest, SettingsListResponse
from harmos._wire.host.v1.status_pb2 import (
    HostIdentity,
    HostStatus,
    StatusGetRequest,
    StatusGetResponse,
    StatusWatchRequest,
    StatusWatchResponse,
)
from harmos._wire.plugin.v1.instance_pb2 import InstanceGetRequest, InstanceStopRequest
from harmos._wire.plugin.v1.instance_pb2_grpc import InstanceStub
from tests.support import calls

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Examples(IsolatedAsyncioTestCase):
    async def test_dynamic_example_discovers_the_fingerprint_and_creates_an_invocation(self) -> None:
        server = grpc.aio.server()
        contract = DiscoveredContract(id="example.echo", fingerprint=b"expected-contract")

        async def discover(request: OperationsGetRequest, context: ServicerContext) -> OperationsGetResponse:
            self.assertEqual(request.id, contract.id)
            return OperationsGetResponse(operation=DiscoveredOperation(contract=contract))

        async def create(request: InvocationsCreateRequest, context: ServicerContext) -> InvocationsCreateResponse:
            self.assertEqual(request.operation, contract.id)
            self.assertEqual(request.fingerprint, contract.fingerprint)
            payload = BytesValue.FromString(request.payload)
            response = BytesValue(value=b"accepted:" + payload.value)
            return InvocationsCreateResponse(payload=response.SerializeToString())

        server.add_generic_rpc_handlers(
            (
                grpc.method_handlers_generic_handler(
                    "harmos.host.v1.Operations",
                    {
                        "Get": grpc.unary_unary_rpc_method_handler(
                            discover,
                            request_deserializer=OperationsGetRequest.FromString,
                            response_serializer=OperationsGetResponse.SerializeToString,
                        )
                    },
                ),
                grpc.method_handlers_generic_handler(
                    "harmos.host.v1.Invocations",
                    {
                        "Create": grpc.unary_unary_rpc_method_handler(
                            create,
                            request_deserializer=InvocationsCreateRequest.FromString,
                            response_serializer=InvocationsCreateResponse.SerializeToString,
                        )
                    },
                ),
            )
        )
        port = server.add_insecure_port("127.0.0.1:0")
        await server.start()
        self.addAsyncCleanup(server.stop, None)
        payload = BytesValue(value=b"dynamic")
        response = await invoke(f"127.0.0.1:{port}", contract.id, payload.SerializeToString())
        self.assertEqual(BytesValue.FromString(response).value, b"accepted:" + payload.value)

    async def test_client_example_discovers_calls_and_streams_from_host(self) -> None:
        # The Host Fixture Owns the Sequence and Completes on Demand Cancellation.
        requested = Event()
        closed = Event()
        server = grpc.aio.server()
        identity = HostIdentity(host_id="fixture")

        async def discover(request: OperationsListRequest, context: ServicerContext) -> OperationsListResponse:
            return OperationsListResponse(operations=[DiscoveredOperation(owner="fixture", grpc_available=True)])

        async def get(request: StatusGetRequest, context: ServicerContext) -> StatusGetResponse:
            return StatusGetResponse(status=HostStatus(identity=identity))

        async def watch(request: StatusWatchRequest, context: ServicerContext) -> AsyncIterator[StatusWatchResponse]:
            requested.set()
            try:
                yield StatusWatchResponse(status=HostStatus(identity=identity), resolved_at=Position(sequence=1))
                await Event().wait()
            finally:
                closed.set()

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
                grpc.method_handlers_generic_handler(
                    "harmos.host.v1.Status",
                    {
                        "Get": grpc.unary_unary_rpc_method_handler(
                            get,
                            request_deserializer=StatusGetRequest.FromString,
                            response_serializer=StatusGetResponse.SerializeToString,
                        ),
                        "Watch": grpc.unary_stream_rpc_method_handler(
                            watch,
                            request_deserializer=StatusWatchRequest.FromString,
                            response_serializer=StatusWatchResponse.SerializeToString,
                        ),
                    },
                ),
            )
        )
        port = server.add_insecure_port("127.0.0.1:0")
        await server.start()
        self.addAsyncCleanup(server.stop, None)

        operations, snapshot, update = await run(f"127.0.0.1:{port}")
        await requested.wait()
        await closed.wait()
        self.assertEqual(len(operations), 1)
        self.assertEqual(snapshot.status.identity, identity)
        self.assertEqual(update.status.identity, identity)
        self.assertEqual(update.resolved_at.sequence, 1)

    async def test_plugin_example_serves_one_operation_to_host(self) -> None:
        # Run the Actual Program; Its Bound Endpoint Is the Readiness Handshake.
        program = Path(__file__).resolve().parents[2] / "examples/plugin.py"
        process = await asyncio.create_subprocess_exec(
            sys.executable,
            str(program),
            "--address",
            "127.0.0.1:0",
            stdout=PIPE,
            stderr=PIPE,
            creationflags=spawn_flags(),
        )
        try:
            assert process.stdout is not None
            # This Bounds a Transport Startup Hang; It Does Not Advance Story Time.
            address = (await asyncio.wait_for(process.stdout.readline(), timeout=30)).decode().strip()
            if not address:
                stdout, stderr = await process.communicate()
                self.fail(stdout.decode() + stderr.decode())
            channel = grpc.aio.insecure_channel(address)
            self.addAsyncCleanup(channel.close)
            host = InstanceStub(channel)
            description = await host.Get(InstanceGetRequest(revision=1))
            self.assertEqual(len(description.declarations.operations), 1)
            self.assertEqual(description.declarations.operations[0].id, "example.echo")
            declarations = SettingsDeclarations.FromString(description.declarations.sections[0].payload)
            self.assertEqual(declarations.values[0].default.WhichOneof("value"), "text")

            request = BytesValue(value=b"example")
            call = await calls.invoke(host, "example.echo", request)
            child = (await call.read()).child
            self.assertEqual(child.operation, "harmos.host.v1.plugins.Settings/List")
            self.assertEqual(SettingsListRequest.FromString(child.payload), SettingsListRequest())
            await calls.reply(call, child, SettingsListResponse(value=SettingValues(values={"prefix": SettingValue(text="typed:")})))
            response = await call.read()
            self.assertEqual(BytesValue.FromString(response.response.payload).value, b"typed:" + request.value)
            self.assertIs(await call.read(), EOF)
            await host.Stop(InstanceStopRequest())
        finally:
            if process.returncode is None:
                process.kill()
            await process.communicate()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
