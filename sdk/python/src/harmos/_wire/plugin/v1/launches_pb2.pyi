from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Optional as _Optional

DESCRIPTOR: _descriptor.FileDescriptor

class LaunchesAnnounceRequest(_message.Message):
    __slots__ = ("token", "address")
    TOKEN_FIELD_NUMBER: _ClassVar[int]
    ADDRESS_FIELD_NUMBER: _ClassVar[int]
    token: str
    address: str
    def __init__(self, token: _Optional[str] = ..., address: _Optional[str] = ...) -> None: ...

class LaunchesAnnounceResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...
