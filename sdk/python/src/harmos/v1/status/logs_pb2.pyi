import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from harmos.v1 import types_pb2 as _types_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class LogsListRequest(_message.Message):
    __slots__ = ("at", "page")
    AT_FIELD_NUMBER: _ClassVar[int]
    PAGE_FIELD_NUMBER: _ClassVar[int]
    at: _types_pb2.Position
    page: _types_pb2.PageRequest
    def __init__(self, at: _Optional[_Union[_types_pb2.Position, _Mapping]] = ..., page: _Optional[_Union[_types_pb2.PageRequest, _Mapping]] = ...) -> None: ...

class LogsListResponse(_message.Message):
    __slots__ = ("events", "resolved_at", "next_page_token")
    EVENTS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    NEXT_PAGE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    events: _containers.RepeatedCompositeFieldContainer[Event]
    resolved_at: _types_pb2.Position
    next_page_token: str
    def __init__(self, events: _Optional[_Iterable[_Union[Event, _Mapping]]] = ..., resolved_at: _Optional[_Union[_types_pb2.Position, _Mapping]] = ..., next_page_token: _Optional[str] = ...) -> None: ...

class LogsWatchRequest(_message.Message):
    __slots__ = ("at", "max_bytes", "max_events")
    AT_FIELD_NUMBER: _ClassVar[int]
    MAX_BYTES_FIELD_NUMBER: _ClassVar[int]
    MAX_EVENTS_FIELD_NUMBER: _ClassVar[int]
    at: _types_pb2.Position
    max_bytes: int
    max_events: int
    def __init__(self, at: _Optional[_Union[_types_pb2.Position, _Mapping]] = ..., max_bytes: _Optional[int] = ..., max_events: _Optional[int] = ...) -> None: ...

class LogsWatchResponse(_message.Message):
    __slots__ = ("events", "resolved_at")
    EVENTS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    events: _containers.RepeatedCompositeFieldContainer[Event]
    resolved_at: _types_pb2.Position
    def __init__(self, events: _Optional[_Iterable[_Union[Event, _Mapping]]] = ..., resolved_at: _Optional[_Union[_types_pb2.Position, _Mapping]] = ...) -> None: ...

class Event(_message.Message):
    __slots__ = ("sequence", "observed_at", "level", "target", "message", "fields")
    class FieldsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    OBSERVED_AT_FIELD_NUMBER: _ClassVar[int]
    LEVEL_FIELD_NUMBER: _ClassVar[int]
    TARGET_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    FIELDS_FIELD_NUMBER: _ClassVar[int]
    sequence: int
    observed_at: _timestamp_pb2.Timestamp
    level: str
    target: str
    message: str
    fields: _containers.ScalarMap[str, str]
    def __init__(self, sequence: _Optional[int] = ..., observed_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., level: _Optional[str] = ..., target: _Optional[str] = ..., message: _Optional[str] = ..., fields: _Optional[_Mapping[str, str]] = ...) -> None: ...
