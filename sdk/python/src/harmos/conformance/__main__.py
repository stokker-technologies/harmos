# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Serve the Fixed Conformance Catalog at an Explicit Remote Address."""

# === Imports === #

# Standard Library Imports
import argparse
import asyncio

# Third Party Imports
from google.protobuf.duration_pb2 import Duration

# Local Imports
from harmos import Context, Failure, Operation, Plugin, Publisher, observer, operation, publisher
from harmos.conformance.v1.fixture_pb2 import Value
from harmos.v1 import plugin_pb2 as wire

# Type Checking Imports

# === Public Exports === #

# === Constants === #

HOST_ECHO = Operation("conformance.host.echo", Value, Value)
HOST_PENDING = Operation("conformance.host.pending", Value, Value)
HOST_FAIL = Operation("conformance.host.fail", Value, Value)

# === Types === #

# === Classes === #


class Fixture:
    def __init__(self):
        self.echoes = 0

    @operation("conformance.echo", request=Value, response=Value)
    async def echo(self, ctx: Context, request: Value) -> Value:
        self.echoes += 1
        return request

    @operation("conformance.children", request=Value, response=Value)
    async def children(self, ctx: Context, request: Value) -> Value:
        first, second = await asyncio.gather(ctx.operation(HOST_ECHO, Value(value=b"\x01")), ctx.operation(HOST_ECHO, Value(value=b"\x02")))
        return Value(value=first.value + second.value)

    @operation("conformance.cancel", request=Value, response=Value)
    async def cancel(self, ctx: Context, request: Value) -> Value:
        return await ctx.operation(HOST_PENDING, request)

    @operation("conformance.failure", request=Value, response=Value)
    async def failure(self, ctx: Context, request: Value) -> Value:
        raise Failure(
            wire.Failure(
                classification=wire.CLASSIFICATION_PERMANENT,
                code="fixture_failure",
                message="Fixture Failed",
                action="Use Another Input",
                operation="conformance.failure",
                plugin="conformance",
                cause=wire.Failure(
                    classification=wire.CLASSIFICATION_TRANSIENT,
                    code="fixture_cause",
                    message="Fixture Cause",
                    action="Retry",
                    operation="conformance.host.fail",
                    plugin="host",
                ),
                details={"fixture": "value"},
                diagnostics="Fixture Diagnostics",
            )
        )

    @operation("conformance.child_failure", request=Value, response=Value)
    async def child_failure(self, ctx: Context, request: Value) -> Value:
        return await ctx.operation(HOST_FAIL, request)

    @observer("conformance.ticks", row=Value, interval=Duration(seconds=1))
    async def ticks(self, ctx: Context) -> Value:
        return Value(value=ctx.sequence.to_bytes(8, "little"))

    @publisher("conformance.device", row=Value)
    async def device(self, ctx: Context, publisher: Publisher) -> None:
        await publisher.publish(Value(value=b"device"))
        await ctx.cancelled()


# === Public Functions === #


def plugin() -> Plugin:
    return Plugin(Fixture(), id="conformance", version="1.0.0", requires=[HOST_ECHO, HOST_PENDING, HOST_FAIL])


# === Private Functions === #


async def _main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--address", required=True)
    options = parser.parse_args()
    endpoint = await plugin().start(options.address)
    print(f"harmos listening {endpoint.address}", flush=True)
    try:
        await endpoint.server.wait_for_termination()
    finally:
        await endpoint.close()


# === Entry Point === #

if __name__ == "__main__":
    asyncio.run(_main())
