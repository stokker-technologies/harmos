from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Optional as _Optional

DESCRIPTOR: _descriptor.FileDescriptor

class TransferFraming(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    TRANSFER_FRAMING_UNSPECIFIED: _ClassVar[TransferFraming]
    TRANSFER_FRAMING_BYTES: _ClassVar[TransferFraming]
TRANSFER_FRAMING_UNSPECIFIED: TransferFraming
TRANSFER_FRAMING_BYTES: TransferFraming

class StorageTransferSettings(_message.Message):
    __slots__ = ("chunk_bytes",)
    CHUNK_BYTES_FIELD_NUMBER: _ClassVar[int]
    chunk_bytes: int
    def __init__(self, chunk_bytes: _Optional[int] = ...) -> None: ...

class MessageSettings(_message.Message):
    __slots__ = ("message_bytes",)
    MESSAGE_BYTES_FIELD_NUMBER: _ClassVar[int]
    message_bytes: int
    def __init__(self, message_bytes: _Optional[int] = ...) -> None: ...
