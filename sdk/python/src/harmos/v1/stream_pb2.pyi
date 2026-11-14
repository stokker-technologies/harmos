import datetime

from google.protobuf import duration_pb2 as _duration_pb2
from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class StreamDeclaration(_message.Message):
    __slots__ = ("id", "row", "description", "interval", "interval_setting")
    ID_FIELD_NUMBER: _ClassVar[int]
    ROW_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    INTERVAL_FIELD_NUMBER: _ClassVar[int]
    INTERVAL_SETTING_FIELD_NUMBER: _ClassVar[int]
    id: str
    row: str
    description: str
    interval: _duration_pb2.Duration
    interval_setting: str
    def __init__(self, id: _Optional[str] = ..., row: _Optional[str] = ..., description: _Optional[str] = ..., interval: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ..., interval_setting: _Optional[str] = ...) -> None: ...

class Tick(_message.Message):
    __slots__ = ("at",)
    AT_FIELD_NUMBER: _ClassVar[int]
    at: _timestamp_pb2.Timestamp
    def __init__(self, at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class SamplesSelection(_message.Message):
    __slots__ = ("stream", "scope")
    STREAM_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    stream: str
    scope: str
    def __init__(self, stream: _Optional[str] = ..., scope: _Optional[str] = ...) -> None: ...

class Sample(_message.Message):
    __slots__ = ("at", "row", "sequence")
    AT_FIELD_NUMBER: _ClassVar[int]
    ROW_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    at: _timestamp_pb2.Timestamp
    row: bytes
    sequence: int
    def __init__(self, at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., row: _Optional[bytes] = ..., sequence: _Optional[int] = ...) -> None: ...
