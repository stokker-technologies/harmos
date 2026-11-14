from harmos._wire.host.v1 import schema_pb2 as _schema_pb2
from harmos._wire.model.v1 import callers_pb2 as _callers_pb2
from harmos._wire.model.v1 import position_pb2 as _position_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class SchemasListRequest(_message.Message):
    __slots__ = ("schema",)
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    schema: str
    def __init__(self, schema: _Optional[str] = ...) -> None: ...

class SchemasListResponse(_message.Message):
    __slots__ = ("schemas",)
    SCHEMAS_FIELD_NUMBER: _ClassVar[int]
    schemas: _containers.RepeatedCompositeFieldContainer[SchemaDescription]
    def __init__(self, schemas: _Optional[_Iterable[_Union[SchemaDescription, _Mapping]]] = ...) -> None: ...

class SchemaDescription(_message.Message):
    __slots__ = ("position", "key", "schema")
    POSITION_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    position: _position_pb2.Position
    key: _schema_pb2.SchemaKey
    schema: _schema_pb2.SchemaGraph
    def __init__(self, position: _Optional[_Union[_position_pb2.Position, _Mapping]] = ..., key: _Optional[_Union[_schema_pb2.SchemaKey, _Mapping]] = ..., schema: _Optional[_Union[_schema_pb2.SchemaGraph, _Mapping]] = ...) -> None: ...
