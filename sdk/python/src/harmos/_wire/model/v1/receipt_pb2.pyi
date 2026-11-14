from harmos._wire.model.v1 import position_pb2 as _position_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Receipt(_message.Message):
    __slots__ = ("request_id", "accepted_at", "durable_at", "completed")
    REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_AT_FIELD_NUMBER: _ClassVar[int]
    DURABLE_AT_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_FIELD_NUMBER: _ClassVar[int]
    request_id: str
    accepted_at: _position_pb2.Position
    durable_at: _position_pb2.Position
    completed: bool
    def __init__(self, request_id: _Optional[str] = ..., accepted_at: _Optional[_Union[_position_pb2.Position, _Mapping]] = ..., durable_at: _Optional[_Union[_position_pb2.Position, _Mapping]] = ..., completed: _Optional[bool] = ...) -> None: ...
