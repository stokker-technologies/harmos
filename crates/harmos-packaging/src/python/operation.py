# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Typed Operation Descriptors And Structural Transport Boundaries"""

# === Imports === #

# Standard Library Imports
import re
from collections.abc import AsyncIterator
from dataclasses import dataclass
from typing import Literal, Protocol

# Third Party Imports
from google.protobuf.message import Message

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# Supplied From The Domain Identity Alphabet During Export.
OWNER_INITIAL = "__HARMOS_OWNER_INITIAL__"
OWNER_EXTRA = "__HARMOS_OWNER_EXTRA__"

# === Types === #

Mode = Literal["unary", "client_stream", "server_stream", "bidi"]

# === Classes === #


@dataclass(frozen=True, slots=True)
class Operation[Request: Message, Response: Message, Input: Message, Output: Message]:
    id: str
    generation: int
    mode: Mode
    codec: Literal["protobuf"]
    request: type[Request]
    response: type[Response]
    input: type[Input] | None
    output: type[Output] | None
    grpc_path: str
    rpc_request: type[Message]
    rpc_response: type[Message]
    run_request: type[Message]
    run_response: type[Message]
    contract: str
    fingerprint: str

    @property
    def run_grpc_path(self) -> str:
        service, method = self.grpc_path.rsplit("/", 1)
        return f"{service}/Run{method}"

    def __post_init__(self) -> None:
        if re.fullmatch(r"[0-9a-f]{64}", self.fingerprint) is None:
            raise ValueError("contract fingerprint must be lowercase 64-hex text")
        if (
            not isinstance(self.generation, int)
            or isinstance(self.generation, bool)
            or not 0 <= self.generation <= 18446744073709551615
        ):
            raise ValueError("generation must be an unsigned 64-bit integer")


class OperationError(RuntimeError):
    """A Structured Refusal Reported By An Invoker"""

    def __init__(self, code: int, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message


class ResponseStream[Response: Message, Output: Message](Protocol):
    """Output Items And The Operation's Declared Final Response"""

    def __aiter__(self) -> AsyncIterator[Output]: ...

    async def response(self) -> Response: ...


class OperationHandle[Response: Message, Output: Message](
    ResponseStream[Response, Output], Protocol
):
    """A Started Operation Whose Cancellation Returns Its Final Response"""

    async def cancel(self) -> Response: ...


class Invoker(Protocol):
    """Transport Implementations Own Connections, Limits And Cancellation"""

    async def start[
        Request: Message,
        Response: Message,
        Input: Message,
        Output: Message,
    ](
        self,
        operation: Operation[Request, Response, Input, Output],
        request: Request,
        items: AsyncIterator[Input] | None = None,
    ) -> OperationHandle[Response, Output]: ...

    async def call[Request: Message, Response: Message](
        self,
        operation: Operation[Request, Response, Message, Message],
        request: Request,
    ) -> Response: ...

    async def upload[Request: Message, Response: Message, Input: Message](
        self,
        operation: Operation[Request, Response, Input, Message],
        request: Request,
        items: AsyncIterator[Input],
    ) -> Response: ...

    def download[Request: Message, Response: Message, Output: Message](
        self,
        operation: Operation[Request, Response, Message, Output],
        request: Request,
    ) -> ResponseStream[Response, Output]: ...

    def exchange[Request: Message, Response: Message, Input: Message, Output: Message](
        self,
        operation: Operation[Request, Response, Input, Output],
        request: Request,
        items: AsyncIterator[Input],
    ) -> ResponseStream[Response, Output]: ...


# === Public Functions === #


def owner_segment(value: str) -> bool:
    return (
        bool(value)
        and value[0] in OWNER_INITIAL
        and all(
            character in OWNER_INITIAL or character in OWNER_EXTRA
            for character in value[1:]
        )
    )


# === Private Functions === #

# === Entry Point === #
