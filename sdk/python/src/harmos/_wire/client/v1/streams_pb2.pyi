from harmos.v1 import position_pb2 as _position_pb2
from harmos.v1 import schema_pb2 as _schema_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class StreamsListRequest(_message.Message):
    __slots__ = ("schema",)
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    schema: str
    def __init__(self, schema: _Optional[str] = ...) -> None: ...

class StreamsListResponse(_message.Message):
    __slots__ = ("streams",)
    STREAMS_FIELD_NUMBER: _ClassVar[int]
    streams: _containers.RepeatedCompositeFieldContainer[StreamIdentity]
    def __init__(self, streams: _Optional[_Iterable[_Union[StreamIdentity, _Mapping]]] = ...) -> None: ...

class StreamIdentity(_message.Message):
    __slots__ = ("schema", "scope", "item", "stream", "first")
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    ITEM_FIELD_NUMBER: _ClassVar[int]
    STREAM_FIELD_NUMBER: _ClassVar[int]
    FIRST_FIELD_NUMBER: _ClassVar[int]
    schema: _schema_pb2.SchemaKey
    scope: str
    item: str
    stream: str
    first: _position_pb2.Position
    def __init__(self, schema: _Optional[_Union[_schema_pb2.SchemaKey, _Mapping]] = ..., scope: _Optional[str] = ..., item: _Optional[str] = ..., stream: _Optional[str] = ..., first: _Optional[_Union[_position_pb2.Position, _Mapping]] = ...) -> None: ...
