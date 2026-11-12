from harmos.v1 import types_pb2 as _types_pb2
from harmos.v1 import storage_types_pb2 as _storage_types_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class StorageCreateFileProductionRequest(_message.Message):
    __slots__ = ("metadata", "mutation")
    METADATA_FIELD_NUMBER: _ClassVar[int]
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    metadata: _storage_types_pb2.FileMetadata
    mutation: _storage_types_pb2.MutationId
    def __init__(self, metadata: _Optional[_Union[_storage_types_pb2.FileMetadata, _Mapping]] = ..., mutation: _Optional[_Union[_storage_types_pb2.MutationId, _Mapping]] = ...) -> None: ...

class StorageCreateFileProductionResponse(_message.Message):
    __slots__ = ("production", "receipt")
    PRODUCTION_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    production: _storage_types_pb2.FileProducer
    receipt: _types_pb2.Receipt
    def __init__(self, production: _Optional[_Union[_storage_types_pb2.FileProducer, _Mapping]] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageGetFileProductionRequest(_message.Message):
    __slots__ = ("file",)
    FILE_FIELD_NUMBER: _ClassVar[int]
    file: _storage_types_pb2.StagedFile
    def __init__(self, file: _Optional[_Union[_storage_types_pb2.StagedFile, _Mapping]] = ...) -> None: ...

class StorageGetFileProductionResponse(_message.Message):
    __slots__ = ("production", "writer_generation", "resolved_at", "accepted_length", "durable_length")
    PRODUCTION_FIELD_NUMBER: _ClassVar[int]
    WRITER_GENERATION_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_LENGTH_FIELD_NUMBER: _ClassVar[int]
    DURABLE_LENGTH_FIELD_NUMBER: _ClassVar[int]
    production: _storage_types_pb2.FileProgress
    writer_generation: int
    resolved_at: _types_pb2.ReadCut
    accepted_length: int
    durable_length: int
    def __init__(self, production: _Optional[_Union[_storage_types_pb2.FileProgress, _Mapping]] = ..., writer_generation: _Optional[int] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ..., accepted_length: _Optional[int] = ..., durable_length: _Optional[int] = ...) -> None: ...

class StorageAppendFileBytesRequest(_message.Message):
    __slots__ = ("session", "offset", "data", "mutation")
    SESSION_FIELD_NUMBER: _ClassVar[int]
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    DATA_FIELD_NUMBER: _ClassVar[int]
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    session: _storage_types_pb2.FileSession
    offset: int
    data: bytes
    mutation: _storage_types_pb2.MutationId
    def __init__(self, session: _Optional[_Union[_storage_types_pb2.FileSession, _Mapping]] = ..., offset: _Optional[int] = ..., data: _Optional[bytes] = ..., mutation: _Optional[_Union[_storage_types_pb2.MutationId, _Mapping]] = ...) -> None: ...

class StorageAppendFileBytesResponse(_message.Message):
    __slots__ = ("production", "receipt")
    PRODUCTION_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    production: _storage_types_pb2.FileProducer
    receipt: _types_pb2.Receipt
    def __init__(self, production: _Optional[_Union[_storage_types_pb2.FileProducer, _Mapping]] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageCreateFileWriterRequest(_message.Message):
    __slots__ = ("session", "mutation")
    SESSION_FIELD_NUMBER: _ClassVar[int]
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    session: _storage_types_pb2.FileSession
    mutation: _storage_types_pb2.MutationId
    def __init__(self, session: _Optional[_Union[_storage_types_pb2.FileSession, _Mapping]] = ..., mutation: _Optional[_Union[_storage_types_pb2.MutationId, _Mapping]] = ...) -> None: ...

class StorageCreateFileWriterResponse(_message.Message):
    __slots__ = ("production", "receipt")
    PRODUCTION_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    production: _storage_types_pb2.FileProducer
    receipt: _types_pb2.Receipt
    def __init__(self, production: _Optional[_Union[_storage_types_pb2.FileProducer, _Mapping]] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StoragePublishFileProductionRequest(_message.Message):
    __slots__ = ("session", "length", "sha256", "mutation")
    SESSION_FIELD_NUMBER: _ClassVar[int]
    LENGTH_FIELD_NUMBER: _ClassVar[int]
    SHA256_FIELD_NUMBER: _ClassVar[int]
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    session: _storage_types_pb2.FileSession
    length: int
    sha256: bytes
    mutation: _storage_types_pb2.MutationId
    def __init__(self, session: _Optional[_Union[_storage_types_pb2.FileSession, _Mapping]] = ..., length: _Optional[int] = ..., sha256: _Optional[bytes] = ..., mutation: _Optional[_Union[_storage_types_pb2.MutationId, _Mapping]] = ...) -> None: ...

class StoragePublishFileProductionResponse(_message.Message):
    __slots__ = ("file", "receipt")
    FILE_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    file: _storage_types_pb2.FileReference
    receipt: _types_pb2.Receipt
    def __init__(self, file: _Optional[_Union[_storage_types_pb2.FileReference, _Mapping]] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageDeleteFileProductionRequest(_message.Message):
    __slots__ = ("session", "mutation")
    SESSION_FIELD_NUMBER: _ClassVar[int]
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    session: _storage_types_pb2.FileSession
    mutation: _storage_types_pb2.MutationId
    def __init__(self, session: _Optional[_Union[_storage_types_pb2.FileSession, _Mapping]] = ..., mutation: _Optional[_Union[_storage_types_pb2.MutationId, _Mapping]] = ...) -> None: ...

class StorageDeleteFileProductionResponse(_message.Message):
    __slots__ = ("production", "receipt")
    PRODUCTION_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    production: _storage_types_pb2.FileProgress
    receipt: _types_pb2.Receipt
    def __init__(self, production: _Optional[_Union[_storage_types_pb2.FileProgress, _Mapping]] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageGetFileBytesRequest(_message.Message):
    __slots__ = ("file", "offset", "max_bytes", "at")
    FILE_FIELD_NUMBER: _ClassVar[int]
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    MAX_BYTES_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    file: _storage_types_pb2.FileReference
    offset: int
    max_bytes: int
    at: _types_pb2.ReadCut
    def __init__(self, file: _Optional[_Union[_storage_types_pb2.FileReference, _Mapping]] = ..., offset: _Optional[int] = ..., max_bytes: _Optional[int] = ..., at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageGetFileBytesResponse(_message.Message):
    __slots__ = ("data", "total_length", "sha256", "resolved_at")
    DATA_FIELD_NUMBER: _ClassVar[int]
    TOTAL_LENGTH_FIELD_NUMBER: _ClassVar[int]
    SHA256_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    data: bytes
    total_length: int
    sha256: bytes
    resolved_at: _types_pb2.ReadCut
    def __init__(self, data: _Optional[bytes] = ..., total_length: _Optional[int] = ..., sha256: _Optional[bytes] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...
