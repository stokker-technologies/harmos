import datetime

from google.protobuf import duration_pb2 as _duration_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Status(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    STATUS_UNSPECIFIED: _ClassVar[Status]
    STATUS_READY: _ClassVar[Status]
STATUS_UNSPECIFIED: Status
STATUS_READY: Status

class Value(_message.Message):
    __slots__ = ("value",)
    VALUE_FIELD_NUMBER: _ClassVar[int]
    value: bytes
    def __init__(self, value: _Optional[bytes] = ...) -> None: ...

class PayloadOnly(_message.Message):
    __slots__ = ("value",)
    VALUE_FIELD_NUMBER: _ClassVar[int]
    value: bytes
    def __init__(self, value: _Optional[bytes] = ...) -> None: ...

class Zebra(_message.Message):
    __slots__ = ("value",)
    VALUE_FIELD_NUMBER: _ClassVar[int]
    value: Value
    def __init__(self, value: _Optional[_Union[Value, _Mapping]] = ...) -> None: ...

class Graph(_message.Message):
    __slots__ = ("items", "tail", "next", "interval", "status")
    class Nested(_message.Message):
        __slots__ = ("count", "next")
        COUNT_FIELD_NUMBER: _ClassVar[int]
        NEXT_FIELD_NUMBER: _ClassVar[int]
        count: int
        next: Graph
        def __init__(self, count: _Optional[int] = ..., next: _Optional[_Union[Graph, _Mapping]] = ...) -> None: ...
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    TAIL_FIELD_NUMBER: _ClassVar[int]
    NEXT_FIELD_NUMBER: _ClassVar[int]
    INTERVAL_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    items: _containers.RepeatedCompositeFieldContainer[Graph.Nested]
    tail: Zebra
    next: Graph
    interval: _duration_pb2.Duration
    status: Status
    def __init__(self, items: _Optional[_Iterable[_Union[Graph.Nested, _Mapping]]] = ..., tail: _Optional[_Union[Zebra, _Mapping]] = ..., next: _Optional[_Union[Graph, _Mapping]] = ..., interval: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ..., status: _Optional[_Union[Status, str]] = ...) -> None: ...
