from harmos.v1 import types_pb2 as _types_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class CatalogEntriesListRequest(_message.Message):
    __slots__ = ("kind", "schema")
    KIND_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    kind: _types_pb2.CatalogKind
    schema: str
    def __init__(self, kind: _Optional[_Union[_types_pb2.CatalogKind, str]] = ..., schema: _Optional[str] = ...) -> None: ...

class CatalogEntriesListResponse(_message.Message):
    __slots__ = ("entries", "operations", "resolved_at")
    ENTRIES_FIELD_NUMBER: _ClassVar[int]
    OPERATIONS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    entries: bytes
    operations: _containers.RepeatedCompositeFieldContainer[_types_pb2.DiscoveredOperation]
    resolved_at: _types_pb2.ReadCut
    def __init__(self, entries: _Optional[bytes] = ..., operations: _Optional[_Iterable[_Union[_types_pb2.DiscoveredOperation, _Mapping]]] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...
