from google.protobuf import descriptor_pb2 as _descriptor_pb2
from harmos.v1 import operation_pb2 as _operation_pb2
from harmos.v1 import stream_pb2 as _stream_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Catalog(_message.Message):
    __slots__ = ("operations", "requires", "streams", "sections", "digest", "types")
    OPERATIONS_FIELD_NUMBER: _ClassVar[int]
    REQUIRES_FIELD_NUMBER: _ClassVar[int]
    STREAMS_FIELD_NUMBER: _ClassVar[int]
    SECTIONS_FIELD_NUMBER: _ClassVar[int]
    DIGEST_FIELD_NUMBER: _ClassVar[int]
    TYPES_FIELD_NUMBER: _ClassVar[int]
    operations: _containers.RepeatedCompositeFieldContainer[_operation_pb2.OperationContract]
    requires: _containers.RepeatedCompositeFieldContainer[_operation_pb2.Requirement]
    streams: _containers.RepeatedCompositeFieldContainer[_stream_pb2.StreamDeclaration]
    sections: _containers.RepeatedCompositeFieldContainer[Section]
    digest: bytes
    types: _descriptor_pb2.FileDescriptorSet
    def __init__(self, operations: _Optional[_Iterable[_Union[_operation_pb2.OperationContract, _Mapping]]] = ..., requires: _Optional[_Iterable[_Union[_operation_pb2.Requirement, _Mapping]]] = ..., streams: _Optional[_Iterable[_Union[_stream_pb2.StreamDeclaration, _Mapping]]] = ..., sections: _Optional[_Iterable[_Union[Section, _Mapping]]] = ..., digest: _Optional[bytes] = ..., types: _Optional[_Union[_descriptor_pb2.FileDescriptorSet, _Mapping]] = ...) -> None: ...

class Section(_message.Message):
    __slots__ = ("type", "payload")
    TYPE_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    type: str
    payload: bytes
    def __init__(self, type: _Optional[str] = ..., payload: _Optional[bytes] = ...) -> None: ...
