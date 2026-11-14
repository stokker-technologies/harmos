from harmos._wire.host.v1.status import logs_pb2 as _logs_pb2
from harmos._wire.model.v1 import callers_pb2 as _callers_pb2
from harmos._wire.model.v1 import page_pb2 as _page_pb2
from harmos._wire.model.v1 import position_pb2 as _position_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class TraceEventsListRequest(_message.Message):
    __slots__ = ("at", "page")
    AT_FIELD_NUMBER: _ClassVar[int]
    PAGE_FIELD_NUMBER: _ClassVar[int]
    at: _position_pb2.Position
    page: _page_pb2.PageRequest
    def __init__(self, at: _Optional[_Union[_position_pb2.Position, _Mapping]] = ..., page: _Optional[_Union[_page_pb2.PageRequest, _Mapping]] = ...) -> None: ...

class TraceEventsListResponse(_message.Message):
    __slots__ = ("events", "resolved_at", "next_page_token")
    EVENTS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    NEXT_PAGE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    events: _containers.RepeatedCompositeFieldContainer[_logs_pb2.Event]
    resolved_at: _position_pb2.Position
    next_page_token: str
    def __init__(self, events: _Optional[_Iterable[_Union[_logs_pb2.Event, _Mapping]]] = ..., resolved_at: _Optional[_Union[_position_pb2.Position, _Mapping]] = ..., next_page_token: _Optional[str] = ...) -> None: ...

class TraceEventsWatchRequest(_message.Message):
    __slots__ = ("at", "max_bytes", "max_events")
    AT_FIELD_NUMBER: _ClassVar[int]
    MAX_BYTES_FIELD_NUMBER: _ClassVar[int]
    MAX_EVENTS_FIELD_NUMBER: _ClassVar[int]
    at: _position_pb2.Position
    max_bytes: int
    max_events: int
    def __init__(self, at: _Optional[_Union[_position_pb2.Position, _Mapping]] = ..., max_bytes: _Optional[int] = ..., max_events: _Optional[int] = ...) -> None: ...

class TraceEventsWatchResponse(_message.Message):
    __slots__ = ("events", "resolved_at")
    EVENTS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    events: _containers.RepeatedCompositeFieldContainer[_logs_pb2.Event]
    resolved_at: _position_pb2.Position
    def __init__(self, events: _Optional[_Iterable[_Union[_logs_pb2.Event, _Mapping]]] = ..., resolved_at: _Optional[_Union[_position_pb2.Position, _Mapping]] = ...) -> None: ...
