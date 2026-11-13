from harmos.v1 import invocation_pb2 as _invocation_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class InvocationsGetRequest(_message.Message):
    __slots__ = ("instance",)
    INSTANCE_FIELD_NUMBER: _ClassVar[int]
    instance: int
    def __init__(self, instance: _Optional[int] = ...) -> None: ...

class InvocationsGetResponse(_message.Message):
    __slots__ = ("invocation",)
    INVOCATION_FIELD_NUMBER: _ClassVar[int]
    invocation: _invocation_pb2.InstanceState
    def __init__(self, invocation: _Optional[_Union[_invocation_pb2.InstanceState, _Mapping]] = ...) -> None: ...

class InvocationsCancelRequest(_message.Message):
    __slots__ = ("instance", "reason")
    INSTANCE_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    instance: int
    reason: str
    def __init__(self, instance: _Optional[int] = ..., reason: _Optional[str] = ...) -> None: ...

class InvocationsCancelResponse(_message.Message):
    __slots__ = ("invocation",)
    INVOCATION_FIELD_NUMBER: _ClassVar[int]
    invocation: _invocation_pb2.InstanceState
    def __init__(self, invocation: _Optional[_Union[_invocation_pb2.InstanceState, _Mapping]] = ...) -> None: ...

class InvocationsWatchRequest(_message.Message):
    __slots__ = ("instance",)
    INSTANCE_FIELD_NUMBER: _ClassVar[int]
    instance: int
    def __init__(self, instance: _Optional[int] = ...) -> None: ...

class InvocationsWatchResponse(_message.Message):
    __slots__ = ("invocation",)
    INVOCATION_FIELD_NUMBER: _ClassVar[int]
    invocation: _invocation_pb2.InstanceState
    def __init__(self, invocation: _Optional[_Union[_invocation_pb2.InstanceState, _Mapping]] = ...) -> None: ...
