import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from harmos._wire.model.v1 import callers_pb2 as _callers_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class SubscriptionsSubscribeRequest(_message.Message):
    __slots__ = ("stream", "row", "types")
    STREAM_FIELD_NUMBER: _ClassVar[int]
    ROW_FIELD_NUMBER: _ClassVar[int]
    TYPES_FIELD_NUMBER: _ClassVar[int]
    stream: str
    row: str
    types: bytes
    def __init__(self, stream: _Optional[str] = ..., row: _Optional[str] = ..., types: _Optional[bytes] = ...) -> None: ...

class SubscriptionsSubscribeResponse(_message.Message):
    __slots__ = ("row", "missed", "opened")
    ROW_FIELD_NUMBER: _ClassVar[int]
    MISSED_FIELD_NUMBER: _ClassVar[int]
    OPENED_FIELD_NUMBER: _ClassVar[int]
    row: SubscriptionRow
    missed: int
    opened: bool
    def __init__(self, row: _Optional[_Union[SubscriptionRow, _Mapping]] = ..., missed: _Optional[int] = ..., opened: _Optional[bool] = ...) -> None: ...

class SubscriptionRow(_message.Message):
    __slots__ = ("payload", "source", "received")
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    RECEIVED_FIELD_NUMBER: _ClassVar[int]
    payload: bytes
    source: _timestamp_pb2.Timestamp
    received: _timestamp_pb2.Timestamp
    def __init__(self, payload: _Optional[bytes] = ..., source: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., received: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...
