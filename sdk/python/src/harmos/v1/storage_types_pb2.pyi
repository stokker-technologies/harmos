import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from harmos.v1 import types_pb2 as _types_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class FilePhase(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PRODUCING: _ClassVar[FilePhase]
    FINISHED: _ClassVar[FilePhase]
    ABANDONED: _ClassVar[FilePhase]

class CaptureConsistency(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    BOUNDED: _ClassVar[CaptureConsistency]
    STABLE: _ClassVar[CaptureConsistency]

class PayloadEncoding(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PAYLOAD_ENCODING_UNSPECIFIED: _ClassVar[PayloadEncoding]
    POSTCARD_V1: _ClassVar[PayloadEncoding]

class SchemaCardinality(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    REQUIRED: _ClassVar[SchemaCardinality]
    OPTIONAL: _ClassVar[SchemaCardinality]
    REPEATED: _ClassVar[SchemaCardinality]
    VARIANT: _ClassVar[SchemaCardinality]

class SchemaKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    BOOL: _ClassVar[SchemaKind]
    UINT32: _ClassVar[SchemaKind]
    UINT64: _ClassVar[SchemaKind]
    INT32: _ClassVar[SchemaKind]
    INT64: _ClassVar[SchemaKind]
    FLOAT32: _ClassVar[SchemaKind]
    FLOAT64: _ClassVar[SchemaKind]
    STRING: _ClassVar[SchemaKind]
    BYTES: _ClassVar[SchemaKind]
    MESSAGE: _ClassVar[SchemaKind]

class SchemaForm(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    OBJECT: _ClassVar[SchemaForm]
    UNIT: _ClassVar[SchemaForm]
    VALUE: _ClassVar[SchemaForm]
    TUPLE: _ClassVar[SchemaForm]
    EXTERNAL: _ClassVar[SchemaForm]
    INTERNAL: _ClassVar[SchemaForm]
    ADJACENT: _ClassVar[SchemaForm]
    UNTAGGED: _ClassVar[SchemaForm]

class SchemaOmission(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    OMIT_NONE: _ClassVar[SchemaOmission]
    OMIT_EMPTY: _ClassVar[SchemaOmission]

class ObjectVersions(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    LATEST: _ClassVar[ObjectVersions]
    REVISIONS: _ClassVar[ObjectVersions]

class Compare(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    EQ: _ClassVar[Compare]
    NE: _ClassVar[Compare]
    LT: _ClassVar[Compare]
    LE: _ClassVar[Compare]
    GT: _ClassVar[Compare]
    GE: _ClassVar[Compare]

class DownsampleAlgorithm(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    DECIMATE: _ClassVar[DownsampleAlgorithm]
    EXTREMA: _ClassVar[DownsampleAlgorithm]
    LTTB: _ClassVar[DownsampleAlgorithm]
    LTTB_LOG: _ClassVar[DownsampleAlgorithm]

class ColumnKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    COLUMN_KIND_OBJECTS: _ClassVar[ColumnKind]
    COLUMN_KIND_RECORDS: _ClassVar[ColumnKind]
    COLUMN_KIND_STREAM: _ClassVar[ColumnKind]

class StorageFailureKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    STORAGE_FAILURE_UNSPECIFIED: _ClassVar[StorageFailureKind]
    STORAGE_FAILURE_UNSUPPORTED: _ClassVar[StorageFailureKind]
    STORAGE_FAILURE_CONFLICT: _ClassVar[StorageFailureKind]
    STORAGE_FAILURE_EXPIRED: _ClassVar[StorageFailureKind]
    STORAGE_FAILURE_NOT_READY: _ClassVar[StorageFailureKind]
    STORAGE_FAILURE_GAP: _ClassVar[StorageFailureKind]
    STORAGE_FAILURE_EXHAUSTED: _ClassVar[StorageFailureKind]
    STORAGE_FAILURE_INVALID: _ClassVar[StorageFailureKind]
    STORAGE_FAILURE_MISSING_INDEX: _ClassVar[StorageFailureKind]
PRODUCING: FilePhase
FINISHED: FilePhase
ABANDONED: FilePhase
BOUNDED: CaptureConsistency
STABLE: CaptureConsistency
PAYLOAD_ENCODING_UNSPECIFIED: PayloadEncoding
POSTCARD_V1: PayloadEncoding
REQUIRED: SchemaCardinality
OPTIONAL: SchemaCardinality
REPEATED: SchemaCardinality
VARIANT: SchemaCardinality
BOOL: SchemaKind
UINT32: SchemaKind
UINT64: SchemaKind
INT32: SchemaKind
INT64: SchemaKind
FLOAT32: SchemaKind
FLOAT64: SchemaKind
STRING: SchemaKind
BYTES: SchemaKind
MESSAGE: SchemaKind
OBJECT: SchemaForm
UNIT: SchemaForm
VALUE: SchemaForm
TUPLE: SchemaForm
EXTERNAL: SchemaForm
INTERNAL: SchemaForm
ADJACENT: SchemaForm
UNTAGGED: SchemaForm
OMIT_NONE: SchemaOmission
OMIT_EMPTY: SchemaOmission
LATEST: ObjectVersions
REVISIONS: ObjectVersions
EQ: Compare
NE: Compare
LT: Compare
LE: Compare
GT: Compare
GE: Compare
DECIMATE: DownsampleAlgorithm
EXTREMA: DownsampleAlgorithm
LTTB: DownsampleAlgorithm
LTTB_LOG: DownsampleAlgorithm
COLUMN_KIND_OBJECTS: ColumnKind
COLUMN_KIND_RECORDS: ColumnKind
COLUMN_KIND_STREAM: ColumnKind
STORAGE_FAILURE_UNSPECIFIED: StorageFailureKind
STORAGE_FAILURE_UNSUPPORTED: StorageFailureKind
STORAGE_FAILURE_CONFLICT: StorageFailureKind
STORAGE_FAILURE_EXPIRED: StorageFailureKind
STORAGE_FAILURE_NOT_READY: StorageFailureKind
STORAGE_FAILURE_GAP: StorageFailureKind
STORAGE_FAILURE_EXHAUSTED: StorageFailureKind
STORAGE_FAILURE_INVALID: StorageFailureKind
STORAGE_FAILURE_MISSING_INDEX: StorageFailureKind

class StoreHistory(_message.Message):
    __slots__ = ("store", "epoch")
    STORE_FIELD_NUMBER: _ClassVar[int]
    EPOCH_FIELD_NUMBER: _ClassVar[int]
    store: str
    epoch: str
    def __init__(self, store: _Optional[str] = ..., epoch: _Optional[str] = ...) -> None: ...

class StorageCursor(_message.Message):
    __slots__ = ("history", "sequence")
    HISTORY_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    history: StoreHistory
    sequence: int
    def __init__(self, history: _Optional[_Union[StoreHistory, _Mapping]] = ..., sequence: _Optional[int] = ...) -> None: ...

class MutationWindow(_message.Message):
    __slots__ = ("history", "id", "expires_at")
    HISTORY_FIELD_NUMBER: _ClassVar[int]
    ID_FIELD_NUMBER: _ClassVar[int]
    EXPIRES_AT_FIELD_NUMBER: _ClassVar[int]
    history: StoreHistory
    id: str
    expires_at: _timestamp_pb2.Timestamp
    def __init__(self, history: _Optional[_Union[StoreHistory, _Mapping]] = ..., id: _Optional[str] = ..., expires_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class StorageInfoRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class StorageInfoResponse(_message.Message):
    __slots__ = ("accepted", "saved", "mutations", "file_batch_bytes")
    ACCEPTED_FIELD_NUMBER: _ClassVar[int]
    SAVED_FIELD_NUMBER: _ClassVar[int]
    MUTATIONS_FIELD_NUMBER: _ClassVar[int]
    FILE_BATCH_BYTES_FIELD_NUMBER: _ClassVar[int]
    accepted: StorageCursor
    saved: StorageCursor
    mutations: MutationWindow
    file_batch_bytes: int
    def __init__(self, accepted: _Optional[_Union[StorageCursor, _Mapping]] = ..., saved: _Optional[_Union[StorageCursor, _Mapping]] = ..., mutations: _Optional[_Union[MutationWindow, _Mapping]] = ..., file_batch_bytes: _Optional[int] = ...) -> None: ...

class MutationId(_message.Message):
    __slots__ = ("window", "key")
    WINDOW_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    window: str
    key: str
    def __init__(self, window: _Optional[str] = ..., key: _Optional[str] = ...) -> None: ...

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

class StagedFile(_message.Message):
    __slots__ = ("history", "id")
    HISTORY_FIELD_NUMBER: _ClassVar[int]
    ID_FIELD_NUMBER: _ClassVar[int]
    history: StoreHistory
    id: str
    def __init__(self, history: _Optional[_Union[StoreHistory, _Mapping]] = ..., id: _Optional[str] = ...) -> None: ...

class FileSession(_message.Message):
    __slots__ = ("history", "id", "generation")
    HISTORY_FIELD_NUMBER: _ClassVar[int]
    ID_FIELD_NUMBER: _ClassVar[int]
    GENERATION_FIELD_NUMBER: _ClassVar[int]
    history: StoreHistory
    id: str
    generation: int
    def __init__(self, history: _Optional[_Union[StoreHistory, _Mapping]] = ..., id: _Optional[str] = ..., generation: _Optional[int] = ...) -> None: ...

class FileProducer(_message.Message):
    __slots__ = ("session", "length")
    SESSION_FIELD_NUMBER: _ClassVar[int]
    LENGTH_FIELD_NUMBER: _ClassVar[int]
    session: FileSession
    length: int
    def __init__(self, session: _Optional[_Union[FileSession, _Mapping]] = ..., length: _Optional[int] = ...) -> None: ...

class FileReference(_message.Message):
    __slots__ = ("position", "metadata", "length", "sha256")
    POSITION_FIELD_NUMBER: _ClassVar[int]
    METADATA_FIELD_NUMBER: _ClassVar[int]
    LENGTH_FIELD_NUMBER: _ClassVar[int]
    SHA256_FIELD_NUMBER: _ClassVar[int]
    position: StorageCursor
    metadata: FileMetadata
    length: int
    sha256: bytes
    def __init__(self, position: _Optional[_Union[StorageCursor, _Mapping]] = ..., metadata: _Optional[_Union[FileMetadata, _Mapping]] = ..., length: _Optional[int] = ..., sha256: _Optional[bytes] = ...) -> None: ...

class FileProgress(_message.Message):
    __slots__ = ("length", "phase")
    LENGTH_FIELD_NUMBER: _ClassVar[int]
    PHASE_FIELD_NUMBER: _ClassVar[int]
    length: int
    phase: FilePhase
    def __init__(self, length: _Optional[int] = ..., phase: _Optional[_Union[FilePhase, str]] = ...) -> None: ...

class StorageDeleteFileRequest(_message.Message):
    __slots__ = ("mutation", "file")
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    FILE_FIELD_NUMBER: _ClassVar[int]
    mutation: MutationId
    file: StagedFile
    def __init__(self, mutation: _Optional[_Union[MutationId, _Mapping]] = ..., file: _Optional[_Union[StagedFile, _Mapping]] = ...) -> None: ...

class StorageDeleteFileResponse(_message.Message):
    __slots__ = ("file", "receipt")
    FILE_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    file: StagedFile
    receipt: _types_pb2.Receipt
    def __init__(self, file: _Optional[_Union[StagedFile, _Mapping]] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class DirectoryRule(_message.Message):
    __slots__ = ("pattern", "include")
    PATTERN_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_FIELD_NUMBER: _ClassVar[int]
    pattern: str
    include: bool
    def __init__(self, pattern: _Optional[str] = ..., include: _Optional[bool] = ...) -> None: ...

class DirectorySelection(_message.Message):
    __slots__ = ("root", "recursive", "rules")
    ROOT_FIELD_NUMBER: _ClassVar[int]
    RECURSIVE_FIELD_NUMBER: _ClassVar[int]
    RULES_FIELD_NUMBER: _ClassVar[int]
    root: str
    recursive: bool
    rules: _containers.RepeatedCompositeFieldContainer[DirectoryRule]
    def __init__(self, root: _Optional[str] = ..., recursive: _Optional[bool] = ..., rules: _Optional[_Iterable[_Union[DirectoryRule, _Mapping]]] = ...) -> None: ...

class DirectoryEntry(_message.Message):
    __slots__ = ("path", "directory", "length")
    PATH_FIELD_NUMBER: _ClassVar[int]
    DIRECTORY_FIELD_NUMBER: _ClassVar[int]
    LENGTH_FIELD_NUMBER: _ClassVar[int]
    path: str
    directory: bool
    length: int
    def __init__(self, path: _Optional[str] = ..., directory: _Optional[bool] = ..., length: _Optional[int] = ...) -> None: ...

class DirectoryCompletion(_message.Message):
    __slots__ = ("entries",)
    ENTRIES_FIELD_NUMBER: _ClassVar[int]
    entries: int
    def __init__(self, entries: _Optional[int] = ...) -> None: ...

class DirectoryRename(_message.Message):
    __slots__ = ("to",)
    FROM_FIELD_NUMBER: _ClassVar[int]
    TO_FIELD_NUMBER: _ClassVar[int]
    to: str
    def __init__(self, to: _Optional[str] = ..., **kwargs) -> None: ...

class Empty(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class DirectoryEvent(_message.Message):
    __slots__ = ("existing", "initial_scan_complete", "created", "modified", "removed", "renamed", "rescan_required")
    EXISTING_FIELD_NUMBER: _ClassVar[int]
    INITIAL_SCAN_COMPLETE_FIELD_NUMBER: _ClassVar[int]
    CREATED_FIELD_NUMBER: _ClassVar[int]
    MODIFIED_FIELD_NUMBER: _ClassVar[int]
    REMOVED_FIELD_NUMBER: _ClassVar[int]
    RENAMED_FIELD_NUMBER: _ClassVar[int]
    RESCAN_REQUIRED_FIELD_NUMBER: _ClassVar[int]
    existing: DirectoryEntry
    initial_scan_complete: Empty
    created: DirectoryEntry
    modified: DirectoryEntry
    removed: str
    renamed: DirectoryRename
    rescan_required: Empty
    def __init__(self, existing: _Optional[_Union[DirectoryEntry, _Mapping]] = ..., initial_scan_complete: _Optional[_Union[Empty, _Mapping]] = ..., created: _Optional[_Union[DirectoryEntry, _Mapping]] = ..., modified: _Optional[_Union[DirectoryEntry, _Mapping]] = ..., removed: _Optional[str] = ..., renamed: _Optional[_Union[DirectoryRename, _Mapping]] = ..., rescan_required: _Optional[_Union[Empty, _Mapping]] = ...) -> None: ...

class FileSource(_message.Message):
    __slots__ = ("stored", "local")
    STORED_FIELD_NUMBER: _ClassVar[int]
    LOCAL_FIELD_NUMBER: _ClassVar[int]
    stored: FileReference
    local: str
    def __init__(self, stored: _Optional[_Union[FileReference, _Mapping]] = ..., local: _Optional[str] = ...) -> None: ...

class ByteRange(_message.Message):
    __slots__ = ("offset", "limit")
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    LIMIT_FIELD_NUMBER: _ClassVar[int]
    offset: int
    limit: int
    def __init__(self, offset: _Optional[int] = ..., limit: _Optional[int] = ...) -> None: ...

class FileBatch(_message.Message):
    __slots__ = ("offset", "bytes")
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    BYTES_FIELD_NUMBER: _ClassVar[int]
    offset: int
    bytes: bytes
    def __init__(self, offset: _Optional[int] = ..., bytes: _Optional[bytes] = ...) -> None: ...

class FileCompletion(_message.Message):
    __slots__ = ("bytes",)
    BYTES_FIELD_NUMBER: _ClassVar[int]
    bytes: int
    def __init__(self, bytes: _Optional[int] = ...) -> None: ...

class FileHash(_message.Message):
    __slots__ = ("bytes_read", "sha256")
    BYTES_READ_FIELD_NUMBER: _ClassVar[int]
    SHA256_FIELD_NUMBER: _ClassVar[int]
    bytes_read: int
    sha256: bytes
    def __init__(self, bytes_read: _Optional[int] = ..., sha256: _Optional[bytes] = ...) -> None: ...

class FollowStart(_message.Message):
    __slots__ = ("beginning", "end", "at")
    BEGINNING_FIELD_NUMBER: _ClassVar[int]
    END_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    beginning: Empty
    end: Empty
    at: int
    def __init__(self, beginning: _Optional[_Union[Empty, _Mapping]] = ..., end: _Optional[_Union[Empty, _Mapping]] = ..., at: _Optional[int] = ...) -> None: ...

class FileEvent(_message.Message):
    __slots__ = ("data", "truncated", "replaced")
    DATA_FIELD_NUMBER: _ClassVar[int]
    TRUNCATED_FIELD_NUMBER: _ClassVar[int]
    REPLACED_FIELD_NUMBER: _ClassVar[int]
    data: FileBatch
    truncated: int
    replaced: Empty
    def __init__(self, data: _Optional[_Union[FileBatch, _Mapping]] = ..., truncated: _Optional[int] = ..., replaced: _Optional[_Union[Empty, _Mapping]] = ...) -> None: ...

class SchemaKey(_message.Message):
    __slots__ = ("id", "version", "payload_encoding")
    ID_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_ENCODING_FIELD_NUMBER: _ClassVar[int]
    id: str
    version: int
    payload_encoding: PayloadEncoding
    def __init__(self, id: _Optional[str] = ..., version: _Optional[int] = ..., payload_encoding: _Optional[_Union[PayloadEncoding, str]] = ...) -> None: ...

class SchemaField(_message.Message):
    __slots__ = ("name", "tag", "kind", "cardinality", "definition")
    NAME_FIELD_NUMBER: _ClassVar[int]
    TAG_FIELD_NUMBER: _ClassVar[int]
    KIND_FIELD_NUMBER: _ClassVar[int]
    CARDINALITY_FIELD_NUMBER: _ClassVar[int]
    DEFINITION_FIELD_NUMBER: _ClassVar[int]
    name: str
    tag: int
    kind: SchemaKind
    cardinality: SchemaCardinality
    definition: int
    def __init__(self, name: _Optional[str] = ..., tag: _Optional[int] = ..., kind: _Optional[_Union[SchemaKind, str]] = ..., cardinality: _Optional[_Union[SchemaCardinality, str]] = ..., definition: _Optional[int] = ...) -> None: ...

class SchemaDefinition(_message.Message):
    __slots__ = ("fields",)
    FIELDS_FIELD_NUMBER: _ClassVar[int]
    fields: _containers.RepeatedCompositeFieldContainer[SchemaField]
    def __init__(self, fields: _Optional[_Iterable[_Union[SchemaField, _Mapping]]] = ...) -> None: ...

class SchemaRepresentation(_message.Message):
    __slots__ = ("definition", "form", "tag", "content", "omissions", "maps")
    class OmissionsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: SchemaOmission
        def __init__(self, key: _Optional[str] = ..., value: _Optional[_Union[SchemaOmission, str]] = ...) -> None: ...
    DEFINITION_FIELD_NUMBER: _ClassVar[int]
    FORM_FIELD_NUMBER: _ClassVar[int]
    TAG_FIELD_NUMBER: _ClassVar[int]
    CONTENT_FIELD_NUMBER: _ClassVar[int]
    OMISSIONS_FIELD_NUMBER: _ClassVar[int]
    MAPS_FIELD_NUMBER: _ClassVar[int]
    definition: int
    form: SchemaForm
    tag: str
    content: str
    omissions: _containers.ScalarMap[str, SchemaOmission]
    maps: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, definition: _Optional[int] = ..., form: _Optional[_Union[SchemaForm, str]] = ..., tag: _Optional[str] = ..., content: _Optional[str] = ..., omissions: _Optional[_Mapping[str, SchemaOmission]] = ..., maps: _Optional[_Iterable[str]] = ...) -> None: ...

class SchemaGraph(_message.Message):
    __slots__ = ("root", "definitions", "representations")
    ROOT_FIELD_NUMBER: _ClassVar[int]
    DEFINITIONS_FIELD_NUMBER: _ClassVar[int]
    REPRESENTATIONS_FIELD_NUMBER: _ClassVar[int]
    root: int
    definitions: _containers.RepeatedCompositeFieldContainer[SchemaDefinition]
    representations: _containers.RepeatedCompositeFieldContainer[SchemaRepresentation]
    def __init__(self, root: _Optional[int] = ..., definitions: _Optional[_Iterable[_Union[SchemaDefinition, _Mapping]]] = ..., representations: _Optional[_Iterable[_Union[SchemaRepresentation, _Mapping]]] = ...) -> None: ...

class SchemaInput(_message.Message):
    __slots__ = ("key", "graph")
    KEY_FIELD_NUMBER: _ClassVar[int]
    GRAPH_FIELD_NUMBER: _ClassVar[int]
    key: SchemaKey
    graph: SchemaGraph
    def __init__(self, key: _Optional[_Union[SchemaKey, _Mapping]] = ..., graph: _Optional[_Union[SchemaGraph, _Mapping]] = ...) -> None: ...

class StorageSetSchemaRequest(_message.Message):
    __slots__ = ("mutation", "schema", "expected_revision", "create_if_absent")
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_REVISION_FIELD_NUMBER: _ClassVar[int]
    CREATE_IF_ABSENT_FIELD_NUMBER: _ClassVar[int]
    mutation: MutationId
    schema: SchemaInput
    expected_revision: bytes
    create_if_absent: bool
    def __init__(self, mutation: _Optional[_Union[MutationId, _Mapping]] = ..., schema: _Optional[_Union[SchemaInput, _Mapping]] = ..., expected_revision: _Optional[bytes] = ..., create_if_absent: _Optional[bool] = ...) -> None: ...

class StorageSetSchemaResponse(_message.Message):
    __slots__ = ("schema", "revision", "receipt")
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    schema: SchemaInput
    revision: bytes
    receipt: _types_pb2.Receipt
    def __init__(self, schema: _Optional[_Union[SchemaInput, _Mapping]] = ..., revision: _Optional[bytes] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageGetSchemaRequest(_message.Message):
    __slots__ = ("key", "at")
    KEY_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    key: SchemaKey
    at: _types_pb2.ReadCut
    def __init__(self, key: _Optional[_Union[SchemaKey, _Mapping]] = ..., at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageGetSchemaResponse(_message.Message):
    __slots__ = ("schema", "revision", "exists", "resolved_at")
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    EXISTS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    schema: SchemaInput
    revision: bytes
    exists: bool
    resolved_at: _types_pb2.ReadCut
    def __init__(self, schema: _Optional[_Union[SchemaInput, _Mapping]] = ..., revision: _Optional[bytes] = ..., exists: _Optional[bool] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class RowSlice(_message.Message):
    __slots__ = ("offset", "limit")
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    LIMIT_FIELD_NUMBER: _ClassVar[int]
    offset: int
    limit: int
    def __init__(self, offset: _Optional[int] = ..., limit: _Optional[int] = ...) -> None: ...

class ObjectSelection(_message.Message):
    __slots__ = ("schema", "key", "versions")
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    VERSIONS_FIELD_NUMBER: _ClassVar[int]
    schema: SchemaKey
    key: str
    versions: ObjectVersions
    def __init__(self, schema: _Optional[_Union[SchemaKey, _Mapping]] = ..., key: _Optional[str] = ..., versions: _Optional[_Union[ObjectVersions, str]] = ...) -> None: ...

class RecordSelection(_message.Message):
    __slots__ = ("schema", "scope", "item", "source_start", "source_end")
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    ITEM_FIELD_NUMBER: _ClassVar[int]
    SOURCE_START_FIELD_NUMBER: _ClassVar[int]
    SOURCE_END_FIELD_NUMBER: _ClassVar[int]
    schema: SchemaKey
    scope: str
    item: str
    source_start: _timestamp_pb2.Timestamp
    source_end: _timestamp_pb2.Timestamp
    def __init__(self, schema: _Optional[_Union[SchemaKey, _Mapping]] = ..., scope: _Optional[str] = ..., item: _Optional[str] = ..., source_start: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., source_end: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class StreamSelection(_message.Message):
    __slots__ = ("schema", "scope", "item", "stream", "source_start", "source_end", "after")
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    ITEM_FIELD_NUMBER: _ClassVar[int]
    STREAM_FIELD_NUMBER: _ClassVar[int]
    SOURCE_START_FIELD_NUMBER: _ClassVar[int]
    SOURCE_END_FIELD_NUMBER: _ClassVar[int]
    AFTER_FIELD_NUMBER: _ClassVar[int]
    schema: SchemaKey
    scope: str
    item: str
    stream: str
    source_start: _timestamp_pb2.Timestamp
    source_end: _timestamp_pb2.Timestamp
    after: int
    def __init__(self, schema: _Optional[_Union[SchemaKey, _Mapping]] = ..., scope: _Optional[str] = ..., item: _Optional[str] = ..., stream: _Optional[str] = ..., source_start: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., source_end: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., after: _Optional[int] = ...) -> None: ...

class SchemaSelection(_message.Message):
    __slots__ = ("schema",)
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    schema: SchemaKey
    def __init__(self, schema: _Optional[_Union[SchemaKey, _Mapping]] = ...) -> None: ...

class FileSelection(_message.Message):
    __slots__ = ("scope", "item")
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    ITEM_FIELD_NUMBER: _ClassVar[int]
    scope: str
    item: str
    def __init__(self, scope: _Optional[str] = ..., item: _Optional[str] = ...) -> None: ...

class QuerySource(_message.Message):
    __slots__ = ("objects", "records", "stream", "schemas", "files")
    OBJECTS_FIELD_NUMBER: _ClassVar[int]
    RECORDS_FIELD_NUMBER: _ClassVar[int]
    STREAM_FIELD_NUMBER: _ClassVar[int]
    SCHEMAS_FIELD_NUMBER: _ClassVar[int]
    FILES_FIELD_NUMBER: _ClassVar[int]
    objects: ObjectSelection
    records: RecordSelection
    stream: StreamSelection
    schemas: SchemaSelection
    files: FileSelection
    def __init__(self, objects: _Optional[_Union[ObjectSelection, _Mapping]] = ..., records: _Optional[_Union[RecordSelection, _Mapping]] = ..., stream: _Optional[_Union[StreamSelection, _Mapping]] = ..., schemas: _Optional[_Union[SchemaSelection, _Mapping]] = ..., files: _Optional[_Union[FileSelection, _Mapping]] = ...) -> None: ...

class QueryCompletion(_message.Message):
    __slots__ = ("at", "rows")
    AT_FIELD_NUMBER: _ClassVar[int]
    ROWS_FIELD_NUMBER: _ClassVar[int]
    at: StorageCursor
    rows: int
    def __init__(self, at: _Optional[_Union[StorageCursor, _Mapping]] = ..., rows: _Optional[int] = ...) -> None: ...

class QueryOutput(_message.Message):
    __slots__ = ("schema", "batch")
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    BATCH_FIELD_NUMBER: _ClassVar[int]
    schema: bytes
    batch: bytes
    def __init__(self, schema: _Optional[bytes] = ..., batch: _Optional[bytes] = ...) -> None: ...

class FieldPath(_message.Message):
    __slots__ = ("fields",)
    FIELDS_FIELD_NUMBER: _ClassVar[int]
    fields: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, fields: _Optional[_Iterable[str]] = ...) -> None: ...

class ExactNumber(_message.Message):
    __slots__ = ("negative", "numerator", "denominator")
    NEGATIVE_FIELD_NUMBER: _ClassVar[int]
    NUMERATOR_FIELD_NUMBER: _ClassVar[int]
    DENOMINATOR_FIELD_NUMBER: _ClassVar[int]
    negative: bool
    numerator: str
    denominator: str
    def __init__(self, negative: _Optional[bool] = ..., numerator: _Optional[str] = ..., denominator: _Optional[str] = ...) -> None: ...

class ScalarValue(_message.Message):
    __slots__ = ("missing", "null", "boolean", "signed", "unsigned", "floating", "text", "datetime", "exact")
    MISSING_FIELD_NUMBER: _ClassVar[int]
    NULL_FIELD_NUMBER: _ClassVar[int]
    BOOLEAN_FIELD_NUMBER: _ClassVar[int]
    SIGNED_FIELD_NUMBER: _ClassVar[int]
    UNSIGNED_FIELD_NUMBER: _ClassVar[int]
    FLOATING_FIELD_NUMBER: _ClassVar[int]
    TEXT_FIELD_NUMBER: _ClassVar[int]
    DATETIME_FIELD_NUMBER: _ClassVar[int]
    EXACT_FIELD_NUMBER: _ClassVar[int]
    missing: Empty
    null: Empty
    boolean: bool
    signed: int
    unsigned: int
    floating: float
    text: str
    datetime: str
    exact: ExactNumber
    def __init__(self, missing: _Optional[_Union[Empty, _Mapping]] = ..., null: _Optional[_Union[Empty, _Mapping]] = ..., boolean: _Optional[bool] = ..., signed: _Optional[int] = ..., unsigned: _Optional[int] = ..., floating: _Optional[float] = ..., text: _Optional[str] = ..., datetime: _Optional[str] = ..., exact: _Optional[_Union[ExactNumber, _Mapping]] = ...) -> None: ...

class Comparison(_message.Message):
    __slots__ = ("op", "value")
    OP_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    op: Compare
    value: ScalarValue
    def __init__(self, op: _Optional[_Union[Compare, str]] = ..., value: _Optional[_Union[ScalarValue, _Mapping]] = ...) -> None: ...

class ScalarValues(_message.Message):
    __slots__ = ("values",)
    VALUES_FIELD_NUMBER: _ClassVar[int]
    values: _containers.RepeatedCompositeFieldContainer[ScalarValue]
    def __init__(self, values: _Optional[_Iterable[_Union[ScalarValue, _Mapping]]] = ...) -> None: ...

class Condition(_message.Message):
    __slots__ = ("compare", "in_values", "is_null")
    COMPARE_FIELD_NUMBER: _ClassVar[int]
    IN_VALUES_FIELD_NUMBER: _ClassVar[int]
    IS_NULL_FIELD_NUMBER: _ClassVar[int]
    compare: Comparison
    in_values: ScalarValues
    is_null: Empty
    def __init__(self, compare: _Optional[_Union[Comparison, _Mapping]] = ..., in_values: _Optional[_Union[ScalarValues, _Mapping]] = ..., is_null: _Optional[_Union[Empty, _Mapping]] = ...) -> None: ...

class Term(_message.Message):
    __slots__ = ("field", "condition", "negate")
    FIELD_FIELD_NUMBER: _ClassVar[int]
    CONDITION_FIELD_NUMBER: _ClassVar[int]
    NEGATE_FIELD_NUMBER: _ClassVar[int]
    field: FieldPath
    condition: Condition
    negate: bool
    def __init__(self, field: _Optional[_Union[FieldPath, _Mapping]] = ..., condition: _Optional[_Union[Condition, _Mapping]] = ..., negate: _Optional[bool] = ...) -> None: ...

class Clause(_message.Message):
    __slots__ = ("terms",)
    TERMS_FIELD_NUMBER: _ClassVar[int]
    terms: _containers.RepeatedCompositeFieldContainer[Term]
    def __init__(self, terms: _Optional[_Iterable[_Union[Term, _Mapping]]] = ...) -> None: ...

class Filter(_message.Message):
    __slots__ = ("clauses",)
    CLAUSES_FIELD_NUMBER: _ClassVar[int]
    clauses: _containers.RepeatedCompositeFieldContainer[Clause]
    def __init__(self, clauses: _Optional[_Iterable[_Union[Clause, _Mapping]]] = ...) -> None: ...

class Projection(_message.Message):
    __slots__ = ("field", "name")
    FIELD_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    field: FieldPath
    name: str
    def __init__(self, field: _Optional[_Union[FieldPath, _Mapping]] = ..., name: _Optional[str] = ...) -> None: ...

class Project(_message.Message):
    __slots__ = ("fields",)
    FIELDS_FIELD_NUMBER: _ClassVar[int]
    fields: _containers.RepeatedCompositeFieldContainer[Projection]
    def __init__(self, fields: _Optional[_Iterable[_Union[Projection, _Mapping]]] = ...) -> None: ...

class Statistic(_message.Message):
    __slots__ = ("count", "sum", "mean", "median", "min", "max", "rms", "std", "percentile")
    COUNT_FIELD_NUMBER: _ClassVar[int]
    SUM_FIELD_NUMBER: _ClassVar[int]
    MEAN_FIELD_NUMBER: _ClassVar[int]
    MEDIAN_FIELD_NUMBER: _ClassVar[int]
    MIN_FIELD_NUMBER: _ClassVar[int]
    MAX_FIELD_NUMBER: _ClassVar[int]
    RMS_FIELD_NUMBER: _ClassVar[int]
    STD_FIELD_NUMBER: _ClassVar[int]
    PERCENTILE_FIELD_NUMBER: _ClassVar[int]
    count: Empty
    sum: Empty
    mean: Empty
    median: Empty
    min: Empty
    max: Empty
    rms: Empty
    std: Empty
    percentile: int
    def __init__(self, count: _Optional[_Union[Empty, _Mapping]] = ..., sum: _Optional[_Union[Empty, _Mapping]] = ..., mean: _Optional[_Union[Empty, _Mapping]] = ..., median: _Optional[_Union[Empty, _Mapping]] = ..., min: _Optional[_Union[Empty, _Mapping]] = ..., max: _Optional[_Union[Empty, _Mapping]] = ..., rms: _Optional[_Union[Empty, _Mapping]] = ..., std: _Optional[_Union[Empty, _Mapping]] = ..., percentile: _Optional[int] = ...) -> None: ...

class Measure(_message.Message):
    __slots__ = ("field", "statistic", "name")
    FIELD_FIELD_NUMBER: _ClassVar[int]
    STATISTIC_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    field: FieldPath
    statistic: Statistic
    name: str
    def __init__(self, field: _Optional[_Union[FieldPath, _Mapping]] = ..., statistic: _Optional[_Union[Statistic, _Mapping]] = ..., name: _Optional[str] = ...) -> None: ...

class Aggregate(_message.Message):
    __slots__ = ("groups", "measures")
    GROUPS_FIELD_NUMBER: _ClassVar[int]
    MEASURES_FIELD_NUMBER: _ClassVar[int]
    groups: _containers.RepeatedCompositeFieldContainer[Projection]
    measures: _containers.RepeatedCompositeFieldContainer[Measure]
    def __init__(self, groups: _Optional[_Iterable[_Union[Projection, _Mapping]]] = ..., measures: _Optional[_Iterable[_Union[Measure, _Mapping]]] = ...) -> None: ...

class Histogram(_message.Message):
    __slots__ = ("field", "edges")
    FIELD_FIELD_NUMBER: _ClassVar[int]
    EDGES_FIELD_NUMBER: _ClassVar[int]
    field: FieldPath
    edges: _containers.RepeatedScalarFieldContainer[float]
    def __init__(self, field: _Optional[_Union[FieldPath, _Mapping]] = ..., edges: _Optional[_Iterable[float]] = ...) -> None: ...

class Downsample(_message.Message):
    __slots__ = ("max_rows", "algorithm", "value", "axis")
    MAX_ROWS_FIELD_NUMBER: _ClassVar[int]
    ALGORITHM_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    AXIS_FIELD_NUMBER: _ClassVar[int]
    max_rows: int
    algorithm: DownsampleAlgorithm
    value: FieldPath
    axis: FieldPath
    def __init__(self, max_rows: _Optional[int] = ..., algorithm: _Optional[_Union[DownsampleAlgorithm, str]] = ..., value: _Optional[_Union[FieldPath, _Mapping]] = ..., axis: _Optional[_Union[FieldPath, _Mapping]] = ...) -> None: ...

class SortKey(_message.Message):
    __slots__ = ("field", "descending", "nulls_first")
    FIELD_FIELD_NUMBER: _ClassVar[int]
    DESCENDING_FIELD_NUMBER: _ClassVar[int]
    NULLS_FIRST_FIELD_NUMBER: _ClassVar[int]
    field: FieldPath
    descending: bool
    nulls_first: bool
    def __init__(self, field: _Optional[_Union[FieldPath, _Mapping]] = ..., descending: _Optional[bool] = ..., nulls_first: _Optional[bool] = ...) -> None: ...

class Sort(_message.Message):
    __slots__ = ("keys",)
    KEYS_FIELD_NUMBER: _ClassVar[int]
    keys: _containers.RepeatedCompositeFieldContainer[SortKey]
    def __init__(self, keys: _Optional[_Iterable[_Union[SortKey, _Mapping]]] = ...) -> None: ...

class Transform(_message.Message):
    __slots__ = ("filter", "project", "aggregate", "histogram", "downsample", "sort", "time_buckets")
    FILTER_FIELD_NUMBER: _ClassVar[int]
    PROJECT_FIELD_NUMBER: _ClassVar[int]
    AGGREGATE_FIELD_NUMBER: _ClassVar[int]
    HISTOGRAM_FIELD_NUMBER: _ClassVar[int]
    DOWNSAMPLE_FIELD_NUMBER: _ClassVar[int]
    SORT_FIELD_NUMBER: _ClassVar[int]
    TIME_BUCKETS_FIELD_NUMBER: _ClassVar[int]
    filter: Filter
    project: Project
    aggregate: Aggregate
    histogram: Histogram
    downsample: Downsample
    sort: Sort
    time_buckets: TimeBucketPlan
    def __init__(self, filter: _Optional[_Union[Filter, _Mapping]] = ..., project: _Optional[_Union[Project, _Mapping]] = ..., aggregate: _Optional[_Union[Aggregate, _Mapping]] = ..., histogram: _Optional[_Union[Histogram, _Mapping]] = ..., downsample: _Optional[_Union[Downsample, _Mapping]] = ..., sort: _Optional[_Union[Sort, _Mapping]] = ..., time_buckets: _Optional[_Union[TimeBucketPlan, _Mapping]] = ...) -> None: ...

class Query(_message.Message):
    __slots__ = ("at", "source", "transforms", "slice")
    AT_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    TRANSFORMS_FIELD_NUMBER: _ClassVar[int]
    SLICE_FIELD_NUMBER: _ClassVar[int]
    at: StorageCursor
    source: QuerySource
    transforms: _containers.RepeatedCompositeFieldContainer[Transform]
    slice: RowSlice
    def __init__(self, at: _Optional[_Union[StorageCursor, _Mapping]] = ..., source: _Optional[_Union[QuerySource, _Mapping]] = ..., transforms: _Optional[_Iterable[_Union[Transform, _Mapping]]] = ..., slice: _Optional[_Union[RowSlice, _Mapping]] = ...) -> None: ...

class StoredRecord(_message.Message):
    __slots__ = ("position", "schema", "scope", "item", "source", "source_time", "payload")
    POSITION_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    ITEM_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    SOURCE_TIME_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    position: StorageCursor
    schema: SchemaKey
    scope: str
    item: str
    source: str
    source_time: _timestamp_pb2.Timestamp
    payload: bytes
    def __init__(self, position: _Optional[_Union[StorageCursor, _Mapping]] = ..., schema: _Optional[_Union[SchemaKey, _Mapping]] = ..., scope: _Optional[str] = ..., item: _Optional[str] = ..., source: _Optional[str] = ..., source_time: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., payload: _Optional[bytes] = ...) -> None: ...

class StoredRow(_message.Message):
    __slots__ = ("record", "stream", "sequence")
    RECORD_FIELD_NUMBER: _ClassVar[int]
    STREAM_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    record: StoredRecord
    stream: str
    sequence: int
    def __init__(self, record: _Optional[_Union[StoredRecord, _Mapping]] = ..., stream: _Optional[str] = ..., sequence: _Optional[int] = ...) -> None: ...

class ObjectVersionToken(_message.Message):
    __slots__ = ("history", "revision")
    HISTORY_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    history: StoreHistory
    revision: int
    def __init__(self, history: _Optional[_Union[StoreHistory, _Mapping]] = ..., revision: _Optional[int] = ...) -> None: ...

class ObjectValue(_message.Message):
    __slots__ = ("schema", "item", "payload")
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    ITEM_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    schema: SchemaKey
    item: str
    payload: bytes
    def __init__(self, schema: _Optional[_Union[SchemaKey, _Mapping]] = ..., item: _Optional[str] = ..., payload: _Optional[bytes] = ...) -> None: ...

class ObjectEntry(_message.Message):
    __slots__ = ("position", "version", "value")
    POSITION_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    position: StorageCursor
    version: ObjectVersionToken
    value: ObjectValue
    def __init__(self, position: _Optional[_Union[StorageCursor, _Mapping]] = ..., version: _Optional[_Union[ObjectVersionToken, _Mapping]] = ..., value: _Optional[_Union[ObjectValue, _Mapping]] = ...) -> None: ...

class DataBatch(_message.Message):
    __slots__ = ("sequence", "destination", "schema", "ipc")
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    DESTINATION_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    IPC_FIELD_NUMBER: _ClassVar[int]
    sequence: int
    destination: str
    schema: SchemaKey
    ipc: bytes
    def __init__(self, sequence: _Optional[int] = ..., destination: _Optional[str] = ..., schema: _Optional[_Union[SchemaKey, _Mapping]] = ..., ipc: _Optional[bytes] = ...) -> None: ...

class ReadLimits(_message.Message):
    __slots__ = ("rows", "bytes")
    ROWS_FIELD_NUMBER: _ClassVar[int]
    BYTES_FIELD_NUMBER: _ClassVar[int]
    rows: int
    bytes: int
    def __init__(self, rows: _Optional[int] = ..., bytes: _Optional[int] = ...) -> None: ...

class StorageCapabilities(_message.Message):
    __slots__ = ("native_arrow", "native_index", "durable_projections", "conditional_objects", "file_sessions", "file_deletion", "mutation_receipts")
    NATIVE_ARROW_FIELD_NUMBER: _ClassVar[int]
    NATIVE_INDEX_FIELD_NUMBER: _ClassVar[int]
    DURABLE_PROJECTIONS_FIELD_NUMBER: _ClassVar[int]
    CONDITIONAL_OBJECTS_FIELD_NUMBER: _ClassVar[int]
    FILE_SESSIONS_FIELD_NUMBER: _ClassVar[int]
    FILE_DELETION_FIELD_NUMBER: _ClassVar[int]
    MUTATION_RECEIPTS_FIELD_NUMBER: _ClassVar[int]
    native_arrow: bool
    native_index: bool
    durable_projections: bool
    conditional_objects: bool
    file_sessions: bool
    file_deletion: bool
    mutation_receipts: bool
    def __init__(self, native_arrow: _Optional[bool] = ..., native_index: _Optional[bool] = ..., durable_projections: _Optional[bool] = ..., conditional_objects: _Optional[bool] = ..., file_sessions: _Optional[bool] = ..., file_deletion: _Optional[bool] = ..., mutation_receipts: _Optional[bool] = ...) -> None: ...

class ColumnSource(_message.Message):
    __slots__ = ("kind", "schema")
    KIND_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    kind: ColumnKind
    schema: SchemaKey
    def __init__(self, kind: _Optional[_Union[ColumnKind, str]] = ..., schema: _Optional[_Union[SchemaKey, _Mapping]] = ...) -> None: ...

class ColumnPredicate(_message.Message):
    __slots__ = ("column", "lower", "upper")
    COLUMN_FIELD_NUMBER: _ClassVar[int]
    LOWER_FIELD_NUMBER: _ClassVar[int]
    UPPER_FIELD_NUMBER: _ClassVar[int]
    column: str
    lower: float
    upper: float
    def __init__(self, column: _Optional[str] = ..., lower: _Optional[float] = ..., upper: _Optional[float] = ...) -> None: ...

class TimeBounds(_message.Message):
    __slots__ = ("column", "lower_ns", "upper_ns")
    COLUMN_FIELD_NUMBER: _ClassVar[int]
    LOWER_NS_FIELD_NUMBER: _ClassVar[int]
    UPPER_NS_FIELD_NUMBER: _ClassVar[int]
    column: str
    lower_ns: int
    upper_ns: int
    def __init__(self, column: _Optional[str] = ..., lower_ns: _Optional[int] = ..., upper_ns: _Optional[int] = ...) -> None: ...

class ColumnSelection(_message.Message):
    __slots__ = ("source", "columns", "predicate", "time", "start", "work")
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    COLUMNS_FIELD_NUMBER: _ClassVar[int]
    PREDICATE_FIELD_NUMBER: _ClassVar[int]
    TIME_FIELD_NUMBER: _ClassVar[int]
    START_FIELD_NUMBER: _ClassVar[int]
    WORK_FIELD_NUMBER: _ClassVar[int]
    source: ColumnSource
    columns: _containers.RepeatedScalarFieldContainer[str]
    predicate: ColumnPredicate
    time: TimeBounds
    start: int
    work: ReadLimits
    def __init__(self, source: _Optional[_Union[ColumnSource, _Mapping]] = ..., columns: _Optional[_Iterable[str]] = ..., predicate: _Optional[_Union[ColumnPredicate, _Mapping]] = ..., time: _Optional[_Union[TimeBounds, _Mapping]] = ..., start: _Optional[int] = ..., work: _Optional[_Union[ReadLimits, _Mapping]] = ...) -> None: ...

class ArrowPage(_message.Message):
    __slots__ = ("ipc", "complete", "scanned", "bytes_read", "columns_read", "payload_rows")
    IPC_FIELD_NUMBER: _ClassVar[int]
    COMPLETE_FIELD_NUMBER: _ClassVar[int]
    SCANNED_FIELD_NUMBER: _ClassVar[int]
    BYTES_READ_FIELD_NUMBER: _ClassVar[int]
    COLUMNS_READ_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_ROWS_FIELD_NUMBER: _ClassVar[int]
    ipc: bytes
    complete: bool
    scanned: int
    bytes_read: int
    columns_read: _containers.RepeatedScalarFieldContainer[str]
    payload_rows: int
    def __init__(self, ipc: _Optional[bytes] = ..., complete: _Optional[bool] = ..., scanned: _Optional[int] = ..., bytes_read: _Optional[int] = ..., columns_read: _Optional[_Iterable[str]] = ..., payload_rows: _Optional[int] = ...) -> None: ...

class IndexDefinition(_message.Message):
    __slots__ = ("name", "version", "source", "columns")
    NAME_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    COLUMNS_FIELD_NUMBER: _ClassVar[int]
    name: str
    version: int
    source: ColumnSource
    columns: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, name: _Optional[str] = ..., version: _Optional[int] = ..., source: _Optional[_Union[ColumnSource, _Mapping]] = ..., columns: _Optional[_Iterable[str]] = ...) -> None: ...

class IndexEntry(_message.Message):
    __slots__ = ("key", "position")
    KEY_FIELD_NUMBER: _ClassVar[int]
    POSITION_FIELD_NUMBER: _ClassVar[int]
    key: bytes
    position: int
    def __init__(self, key: _Optional[bytes] = ..., position: _Optional[int] = ...) -> None: ...

class IndexSelection(_message.Message):
    __slots__ = ("name", "lower", "upper", "prefix", "descending", "work")
    NAME_FIELD_NUMBER: _ClassVar[int]
    LOWER_FIELD_NUMBER: _ClassVar[int]
    UPPER_FIELD_NUMBER: _ClassVar[int]
    PREFIX_FIELD_NUMBER: _ClassVar[int]
    DESCENDING_FIELD_NUMBER: _ClassVar[int]
    WORK_FIELD_NUMBER: _ClassVar[int]
    name: str
    lower: bytes
    upper: bytes
    prefix: bytes
    descending: bool
    work: ReadLimits
    def __init__(self, name: _Optional[str] = ..., lower: _Optional[bytes] = ..., upper: _Optional[bytes] = ..., prefix: _Optional[bytes] = ..., descending: _Optional[bool] = ..., work: _Optional[_Union[ReadLimits, _Mapping]] = ...) -> None: ...

class IndexPage(_message.Message):
    __slots__ = ("entries", "complete")
    ENTRIES_FIELD_NUMBER: _ClassVar[int]
    COMPLETE_FIELD_NUMBER: _ClassVar[int]
    entries: _containers.RepeatedCompositeFieldContainer[IndexEntry]
    complete: bool
    def __init__(self, entries: _Optional[_Iterable[_Union[IndexEntry, _Mapping]]] = ..., complete: _Optional[bool] = ...) -> None: ...

class TimeBucketPlan(_message.Message):
    __slots__ = ("origin_ns", "width_ns", "time", "value", "identity", "group", "max_samples", "max_bytes")
    ORIGIN_NS_FIELD_NUMBER: _ClassVar[int]
    WIDTH_NS_FIELD_NUMBER: _ClassVar[int]
    TIME_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    GROUP_FIELD_NUMBER: _ClassVar[int]
    MAX_SAMPLES_FIELD_NUMBER: _ClassVar[int]
    MAX_BYTES_FIELD_NUMBER: _ClassVar[int]
    origin_ns: int
    width_ns: int
    time: str
    value: str
    identity: str
    group: _containers.RepeatedScalarFieldContainer[str]
    max_samples: int
    max_bytes: int
    def __init__(self, origin_ns: _Optional[int] = ..., width_ns: _Optional[int] = ..., time: _Optional[str] = ..., value: _Optional[str] = ..., identity: _Optional[str] = ..., group: _Optional[_Iterable[str]] = ..., max_samples: _Optional[int] = ..., max_bytes: _Optional[int] = ...) -> None: ...

class SelectColumns(_message.Message):
    __slots__ = ("columns",)
    COLUMNS_FIELD_NUMBER: _ClassVar[int]
    columns: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, columns: _Optional[_Iterable[str]] = ...) -> None: ...

class ProjectionDefinition(_message.Message):
    __slots__ = ("name", "version", "source", "select", "time_buckets")
    NAME_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    SELECT_FIELD_NUMBER: _ClassVar[int]
    TIME_BUCKETS_FIELD_NUMBER: _ClassVar[int]
    name: str
    version: int
    source: ColumnSource
    select: SelectColumns
    time_buckets: TimeBucketPlan
    def __init__(self, name: _Optional[str] = ..., version: _Optional[int] = ..., source: _Optional[_Union[ColumnSource, _Mapping]] = ..., select: _Optional[_Union[SelectColumns, _Mapping]] = ..., time_buckets: _Optional[_Union[TimeBucketPlan, _Mapping]] = ...) -> None: ...

class ProjectionSelection(_message.Message):
    __slots__ = ("name", "revision", "work")
    NAME_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    WORK_FIELD_NUMBER: _ClassVar[int]
    name: str
    revision: bytes
    work: ReadLimits
    def __init__(self, name: _Optional[str] = ..., revision: _Optional[bytes] = ..., work: _Optional[_Union[ReadLimits, _Mapping]] = ...) -> None: ...

class ProjectionPage(_message.Message):
    __slots__ = ("ipc", "changed_keys")
    IPC_FIELD_NUMBER: _ClassVar[int]
    CHANGED_KEYS_FIELD_NUMBER: _ClassVar[int]
    ipc: bytes
    changed_keys: _containers.RepeatedScalarFieldContainer[bytes]
    def __init__(self, ipc: _Optional[bytes] = ..., changed_keys: _Optional[_Iterable[bytes]] = ...) -> None: ...

class ProjectionStatus(_message.Message):
    __slots__ = ("frontier", "revision", "rows", "changed_keys")
    FRONTIER_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    ROWS_FIELD_NUMBER: _ClassVar[int]
    CHANGED_KEYS_FIELD_NUMBER: _ClassVar[int]
    frontier: StorageCursor
    revision: int
    rows: int
    changed_keys: int
    def __init__(self, frontier: _Optional[_Union[StorageCursor, _Mapping]] = ..., revision: _Optional[int] = ..., rows: _Optional[int] = ..., changed_keys: _Optional[int] = ...) -> None: ...

class SavedSnapshot(_message.Message):
    __slots__ = ("token", "at")
    TOKEN_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    token: bytes
    at: StorageCursor
    def __init__(self, token: _Optional[bytes] = ..., at: _Optional[_Union[StorageCursor, _Mapping]] = ...) -> None: ...

class DefinitionRevision(_message.Message):
    __slots__ = ("revision",)
    REVISION_FIELD_NUMBER: _ClassVar[int]
    revision: int
    def __init__(self, revision: _Optional[int] = ...) -> None: ...

class ProjectionState(_message.Message):
    __slots__ = ("status",)
    STATUS_FIELD_NUMBER: _ClassVar[int]
    status: ProjectionStatus
    def __init__(self, status: _Optional[_Union[ProjectionStatus, _Mapping]] = ...) -> None: ...

class StorageFailure(_message.Message):
    __slots__ = ("kind", "message")
    KIND_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    kind: StorageFailureKind
    message: str
    def __init__(self, kind: _Optional[_Union[StorageFailureKind, str]] = ..., message: _Optional[str] = ...) -> None: ...
