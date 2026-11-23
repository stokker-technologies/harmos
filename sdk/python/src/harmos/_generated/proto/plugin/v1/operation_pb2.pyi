from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class AssignedOperation(_message.Message):
    __slots__ = ("operation", "fingerprint", "generation")
    OPERATION_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    GENERATION_FIELD_NUMBER: _ClassVar[int]
    operation: str
    fingerprint: bytes
    generation: int
    def __init__(self, operation: _Optional[str] = ..., fingerprint: _Optional[bytes] = ..., generation: _Optional[int] = ...) -> None: ...

class OperationAssignments(_message.Message):
    __slots__ = ("operations",)
    OPERATIONS_FIELD_NUMBER: _ClassVar[int]
    operations: _containers.RepeatedCompositeFieldContainer[AssignedOperation]
    def __init__(self, operations: _Optional[_Iterable[_Union[AssignedOperation, _Mapping]]] = ...) -> None: ...
