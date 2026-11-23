from harmos._generated.proto.model.v1 import page_pb2 as _page_pb2
from harmos._generated.proto.model.v1 import position_pb2 as _position_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
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

class ListFileProductionsRequest(_message.Message):
    __slots__ = ("scope", "page", "at")
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    PAGE_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    scope: str
    page: _page_pb2.PageRequest
    at: _position_pb2.ReadCut
    def __init__(self, scope: _Optional[str] = ..., page: _Optional[_Union[_page_pb2.PageRequest, _Mapping]] = ..., at: _Optional[_Union[_position_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class ListFileProductionsResponse(_message.Message):
    __slots__ = ("productions", "next_page_token", "resolved_at")
    PRODUCTIONS_FIELD_NUMBER: _ClassVar[int]
    NEXT_PAGE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    productions: _containers.RepeatedCompositeFieldContainer[Upload]
    next_page_token: str
    resolved_at: _position_pb2.ReadCut
    def __init__(self, productions: _Optional[_Iterable[_Union[Upload, _Mapping]]] = ..., next_page_token: _Optional[str] = ..., resolved_at: _Optional[_Union[_position_pb2.ReadCut, _Mapping]] = ...) -> None: ...

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
