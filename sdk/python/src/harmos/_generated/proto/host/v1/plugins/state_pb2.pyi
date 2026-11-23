from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Optional as _Optional

DESCRIPTOR: _descriptor.FileDescriptor

class StateGetRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class StateGetResponse(_message.Message):
    __slots__ = ("value", "revision")
    VALUE_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    value: bytes
    revision: int
    def __init__(self, value: _Optional[bytes] = ..., revision: _Optional[int] = ...) -> None: ...

class StatePutRequest(_message.Message):
    __slots__ = ("expected_revision", "value")
    EXPECTED_REVISION_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    expected_revision: int
    value: bytes
    def __init__(self, expected_revision: _Optional[int] = ..., value: _Optional[bytes] = ...) -> None: ...

class StatePutResponse(_message.Message):
    __slots__ = ("revision",)
    REVISION_FIELD_NUMBER: _ClassVar[int]
    revision: int
    def __init__(self, revision: _Optional[int] = ...) -> None: ...
