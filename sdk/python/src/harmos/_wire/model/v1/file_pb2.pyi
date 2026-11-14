from harmos._wire.model.v1 import position_pb2 as _position_pb2
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class UploadState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    UPLOAD_STATE_UNSPECIFIED: _ClassVar[UploadState]
    UPLOAD_STATE_PRODUCING: _ClassVar[UploadState]
    UPLOAD_STATE_FINISHED: _ClassVar[UploadState]
    UPLOAD_STATE_ABANDONED: _ClassVar[UploadState]
UPLOAD_STATE_UNSPECIFIED: UploadState
UPLOAD_STATE_PRODUCING: UploadState
UPLOAD_STATE_FINISHED: UploadState
UPLOAD_STATE_ABANDONED: UploadState

class FileMetadata(_message.Message):
    __slots__ = ("id", "scope", "media_type", "expected_length", "expected_sha256")
    ID_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    MEDIA_TYPE_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_LENGTH_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_SHA256_FIELD_NUMBER: _ClassVar[int]
    id: str
    scope: str
    media_type: str
    expected_length: int
    expected_sha256: bytes
    def __init__(self, id: _Optional[str] = ..., scope: _Optional[str] = ..., media_type: _Optional[str] = ..., expected_length: _Optional[int] = ..., expected_sha256: _Optional[bytes] = ...) -> None: ...

class FileReference(_message.Message):
    __slots__ = ("position", "metadata", "length", "sha256")
    POSITION_FIELD_NUMBER: _ClassVar[int]
    METADATA_FIELD_NUMBER: _ClassVar[int]
    LENGTH_FIELD_NUMBER: _ClassVar[int]
    SHA256_FIELD_NUMBER: _ClassVar[int]
    position: _position_pb2.Position
    metadata: FileMetadata
    length: int
    sha256: bytes
    def __init__(self, position: _Optional[_Union[_position_pb2.Position, _Mapping]] = ..., metadata: _Optional[_Union[FileMetadata, _Mapping]] = ..., length: _Optional[int] = ..., sha256: _Optional[bytes] = ...) -> None: ...

class File(_message.Message):
    __slots__ = ("reference",)
    REFERENCE_FIELD_NUMBER: _ClassVar[int]
    reference: FileReference
    def __init__(self, reference: _Optional[_Union[FileReference, _Mapping]] = ...) -> None: ...

class Upload(_message.Message):
    __slots__ = ("id", "history", "writer_generation", "accepted_length", "durable_length", "state")
    ID_FIELD_NUMBER: _ClassVar[int]
    HISTORY_FIELD_NUMBER: _ClassVar[int]
    WRITER_GENERATION_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_LENGTH_FIELD_NUMBER: _ClassVar[int]
    DURABLE_LENGTH_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    id: str
    history: _position_pb2.History
    writer_generation: int
    accepted_length: int
    durable_length: int
    state: UploadState
    def __init__(self, id: _Optional[str] = ..., history: _Optional[_Union[_position_pb2.History, _Mapping]] = ..., writer_generation: _Optional[int] = ..., accepted_length: _Optional[int] = ..., durable_length: _Optional[int] = ..., state: _Optional[_Union[UploadState, str]] = ...) -> None: ...
