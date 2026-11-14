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
import unittest
from collections.abc import AsyncIterator
from pathlib import Path

import grpc
from google.protobuf.wrappers_pb2 import BytesValue
from harmos_tools.process import spawn_flags

from examples.client import run
from harmos._wire.client.v1 import operations_pb2 as declarations
from harmos._wire.client.v1 import status_pb2 as status
from harmos.plugin.host.v1.settings_pb2 import SettingsListRequest, SettingsListResponse
from harmos.plugin.v1 import instance_pb2 as carrier
from harmos.plugin.v1.instance_pb2_grpc import InstanceStub
from harmos.v1.operation_pb2 import DiscoveredOperation
from harmos.v1.position_pb2 import Position
from harmos.v1.setting_pb2 import SettingsDeclarations, SettingValue, SettingValues
from tests.support import calls

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Examples(unittest.IsolatedAsyncioTestCase):
    async def test_client_example_discovers_calls_and_streams_from_host(self) -> None:
        # The Host Fixture Owns the Sequence and Completes on Demand Cancellation.
        requested = asyncio.Event()
        closed = asyncio.Event()
        server = grpc.aio.server()
        identity = status.HostIdentity(host_id="fixture")

        async def discover(
            request: declarations.OperationsListRequest, context: grpc.aio.ServicerContext
        ) -> declarations.OperationsListResponse:
            return declarations.OperationsListResponse(operations=[DiscoveredOperation(owner="fixture", grpc_available=True)])

        async def get(request: status.StatusGetRequest, context: grpc.aio.ServicerContext) -> status.StatusGetResponse:
            return status.StatusGetResponse(status=status.HostStatus(identity=identity))

        async def watch(request: status.StatusWatchRequest, context: grpc.aio.ServicerContext) -> AsyncIterator[status.StatusWatchResponse]:
            requested.set()
            try:
                yield status.StatusWatchResponse(status=status.HostStatus(identity=identity), resolved_at=Position(sequence=1))
                await asyncio.Event().wait()
            finally:
                closed.set()

        server.add_generic_rpc_handlers(
            (
                grpc.method_handlers_generic_handler(
                    "harmos.client.v1.Operations",
                    {
                        "List": grpc.unary_unary_rpc_method_handler(
                            discover,
                            request_deserializer=declarations.OperationsListRequest.FromString,
                            response_serializer=declarations.OperationsListResponse.SerializeToString,
                        )
                    },
                ),
                grpc.method_handlers_generic_handler(
                    "harmos.client.v1.Status",
                    {
                        "Get": grpc.unary_unary_rpc_method_handler(
                            get,
                            request_deserializer=status.StatusGetRequest.FromString,
                            response_serializer=status.StatusGetResponse.SerializeToString,
                        ),
                        "Watch": grpc.unary_stream_rpc_method_handler(
                            watch,
                            request_deserializer=status.StatusWatchRequest.FromString,
                            response_serializer=status.StatusWatchResponse.SerializeToString,
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
        self.assertEqual(len(operations.operations), 1)
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
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
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
            description = await host.Get(carrier.InstanceGetRequest(revision=1))
            self.assertEqual(len(description.declarations.operations), 1)
            self.assertEqual(description.declarations.operations[0].id, "example.echo")
            declarations = SettingsDeclarations.FromString(description.declarations.sections[0].payload)
            self.assertEqual(declarations.values[0].default.WhichOneof("value"), "text")

            request = BytesValue(value=b"example")
            call = await calls.invoke(host, "example.echo", request)
            child = (await call.read()).child
            self.assertEqual(child.operation, "harmos.plugin.host.v1.Settings/List")
            self.assertEqual(SettingsListRequest.FromString(child.payload), SettingsListRequest())
            await calls.reply(call, child, SettingsListResponse(value=SettingValues(values={"prefix": SettingValue(text="typed:")})))
            response = await call.read()
            self.assertEqual(BytesValue.FromString(response.response.payload).value, b"typed:" + request.value)
            self.assertIs(await call.read(), grpc.aio.EOF)
            await host.Stop(carrier.InstanceStopRequest())
        finally:
            if process.returncode is None:
                process.kill()
            await process.communicate()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
