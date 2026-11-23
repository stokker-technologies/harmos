# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""A Minimal Host Bootstrap Listener That Receives Managed Launch Announcements."""

# === Imports === #

# Standard Library Imports
import asyncio

# Third Party Imports
import grpc
from grpc.aio import ServicerContext

from harmos._generated.proto.plugin.v1.launches_pb2 import LaunchesAnnounceRequest, LaunchesAnnounceResponse
from harmos._generated.proto.plugin.v1.launches_pb2_grpc import LaunchesServicer, add_LaunchesServicer_to_server

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

TOKEN = "story-launch-token"

# === Types === #

# === Classes === #


class Bootstrap(LaunchesServicer):
    """Serve Announce on a Loopback Port; a Refusal Answers Every Call With UNAUTHENTICATED."""

    def __init__(self, refusal: str | None = None) -> None:
        self.refusal = refusal
        self.announces: asyncio.Queue[LaunchesAnnounceRequest] = asyncio.Queue()
        self.server = grpc.aio.server()
        add_LaunchesServicer_to_server(self, self.server)
        self.endpoint = f"127.0.0.1:{self.server.add_insecure_port('127.0.0.1:0')}"

    async def Announce(self, request: LaunchesAnnounceRequest, context: ServicerContext) -> LaunchesAnnounceResponse:  # noqa: N802
        self.announces.put_nowait(request)
        if self.refusal is not None:
            await context.abort(grpc.StatusCode.UNAUTHENTICATED, self.refusal)
        return LaunchesAnnounceResponse()

    def environment(self, base: dict[str, str]) -> dict[str, str]:
        """Return a Copy of the Environment Carrying Both Launch Variables."""
        return {**base, "HARMOS_HOST_ENDPOINT": self.endpoint, "HARMOS_LAUNCH_TOKEN": TOKEN}

    async def start(self) -> None:
        await self.server.start()

    async def stop(self) -> None:
        await self.server.stop(None)


# === Public Functions === #


async def start(refusal: str | None = None) -> Bootstrap:
    bootstrap = Bootstrap(refusal)
    await bootstrap.start()
    return bootstrap


# === Private Functions === #

# === Entry Point === #
