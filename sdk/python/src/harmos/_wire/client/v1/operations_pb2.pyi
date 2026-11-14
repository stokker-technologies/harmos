from harmos._wire.v1 import operation_pb2 as _operation_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class OperationsListRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class OperationsListResponse(_message.Message):
    __slots__ = ("operations",)
    OPERATIONS_FIELD_NUMBER: _ClassVar[int]
    operations: _containers.RepeatedCompositeFieldContainer[_operation_pb2.DiscoveredOperation]
    def __init__(self, operations: _Optional[_Iterable[_Union[_operation_pb2.DiscoveredOperation, _Mapping]]] = ...) -> None: ...

class OperationsGetRequest(_message.Message):
    __slots__ = ("id",)
    ID_FIELD_NUMBER: _ClassVar[int]
    id: str
    def __init__(self, id: _Optional[str] = ...) -> None: ...

class OperationsGetResponse(_message.Message):
    __slots__ = ("operation",)
    OPERATION_FIELD_NUMBER: _ClassVar[int]
    operation: _operation_pb2.DiscoveredOperation
    def __init__(self, operation: _Optional[_Union[_operation_pb2.DiscoveredOperation, _Mapping]] = ...) -> None: ...
