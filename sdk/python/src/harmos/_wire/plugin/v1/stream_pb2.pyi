import datetime

from google.protobuf import duration_pb2 as _duration_pb2
from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Window(_message.Message):
    __slots__ = ("start", "period", "count", "sequence")
    START_FIELD_NUMBER: _ClassVar[int]
    PERIOD_FIELD_NUMBER: _ClassVar[int]
    COUNT_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    start: _timestamp_pb2.Timestamp
    period: _duration_pb2.Duration
    count: int
    sequence: int
    def __init__(self, start: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., period: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ..., count: _Optional[int] = ..., sequence: _Optional[int] = ...) -> None: ...

class SampleBatch(_message.Message):
    __slots__ = ("offset", "count", "columns", "last")
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    COUNT_FIELD_NUMBER: _ClassVar[int]
    COLUMNS_FIELD_NUMBER: _ClassVar[int]
    LAST_FIELD_NUMBER: _ClassVar[int]
    offset: int
    count: int
    columns: bytes
    last: bool
    def __init__(self, offset: _Optional[int] = ..., count: _Optional[int] = ..., columns: _Optional[bytes] = ..., last: _Optional[bool] = ...) -> None: ...

class CollectionDemand(_message.Message):
    __slots__ = ("streams",)
    STREAMS_FIELD_NUMBER: _ClassVar[int]
    streams: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, streams: _Optional[_Iterable[str]] = ...) -> None: ...

class CollectionDemandResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class CollectionBatch(_message.Message):
    __slots__ = ("offset", "streams", "last")
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    STREAMS_FIELD_NUMBER: _ClassVar[int]
    LAST_FIELD_NUMBER: _ClassVar[int]
    offset: int
    streams: _containers.RepeatedCompositeFieldContainer[CollectionRows]
    last: bool
    def __init__(self, offset: _Optional[int] = ..., streams: _Optional[_Iterable[_Union[CollectionRows, _Mapping]]] = ..., last: _Optional[bool] = ...) -> None: ...

class CollectionRows(_message.Message):
    __slots__ = ("stream", "rows")
    STREAM_FIELD_NUMBER: _ClassVar[int]
    ROWS_FIELD_NUMBER: _ClassVar[int]
    stream: str
    rows: _containers.RepeatedScalarFieldContainer[bytes]
    def __init__(self, stream: _Optional[str] = ..., rows: _Optional[_Iterable[bytes]] = ...) -> None: ...

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
