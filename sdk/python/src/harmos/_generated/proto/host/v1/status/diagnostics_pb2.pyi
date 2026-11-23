from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Optional as _Optional

DESCRIPTOR: _descriptor.FileDescriptor

class DiagnosticsGetRequest(_message.Message):
    __slots__ = ("name", "offset", "max_bytes")
    NAME_FIELD_NUMBER: _ClassVar[int]
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    MAX_BYTES_FIELD_NUMBER: _ClassVar[int]
    name: str
    offset: int
    max_bytes: int
    def __init__(self, name: _Optional[str] = ..., offset: _Optional[int] = ..., max_bytes: _Optional[int] = ...) -> None: ...

class DiagnosticsGetResponse(_message.Message):
    __slots__ = ("name", "data", "total_length", "sha256")
    NAME_FIELD_NUMBER: _ClassVar[int]
    DATA_FIELD_NUMBER: _ClassVar[int]
    TOTAL_LENGTH_FIELD_NUMBER: _ClassVar[int]
    SHA256_FIELD_NUMBER: _ClassVar[int]
    name: str
    data: bytes
    total_length: int
    sha256: bytes
    def __init__(self, name: _Optional[str] = ..., data: _Optional[bytes] = ..., total_length: _Optional[int] = ..., sha256: _Optional[bytes] = ...) -> None: ...
