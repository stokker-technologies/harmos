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
from collections.abc import AsyncIterator
from dataclasses import dataclass
from typing import Literal, Protocol

# Third Party Imports
from google.protobuf.message import Message

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

Mode = Literal["unary", "client_stream", "server_stream", "bidi"]

# === Classes === #


@dataclass(frozen=True, slots=True)
class Operation[Request: Message, Response: Message, Input: Message, Output: Message]:
    id: str
    version: int
    mode: Mode
    codec: Literal["protobuf"]
    request: type[Request]
    response: type[Response]
    input: type[Input] | None
    output: type[Output] | None
    grpc_path: str
    rpc_request: type[Message]
    rpc_response: type[Message]
    contract: str
    fingerprint: str


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


class Invoker(Protocol):
    """Transport Implementations Own Connections, Limits And Cancellation"""

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

# === Private Functions === #

# === Entry Point === #
