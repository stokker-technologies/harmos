from google.protobuf import descriptor_pb2 as _descriptor_pb2
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from typing import ClassVar as _ClassVar

DESCRIPTOR: _descriptor.FileDescriptor

class Callers(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CALLERS_UNSPECIFIED: _ClassVar[Callers]
    CLIENTS: _ClassVar[Callers]
    PLUGINS: _ClassVar[Callers]
    ALL: _ClassVar[Callers]
CALLERS_UNSPECIFIED: Callers
CLIENTS: Callers
PLUGINS: Callers
ALL: Callers
CALLERS_FIELD_NUMBER: _ClassVar[int]
callers: _descriptor.FieldDescriptor
