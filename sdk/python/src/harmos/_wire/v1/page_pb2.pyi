from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Optional as _Optional

DESCRIPTOR: _descriptor.FileDescriptor

class PageRequest(_message.Message):
    __slots__ = ("page_size", "max_bytes", "page_token")
    PAGE_SIZE_FIELD_NUMBER: _ClassVar[int]
    MAX_BYTES_FIELD_NUMBER: _ClassVar[int]
    PAGE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    page_size: int
    max_bytes: int
    page_token: str
    def __init__(self, page_size: _Optional[int] = ..., max_bytes: _Optional[int] = ..., page_token: _Optional[str] = ...) -> None: ...
