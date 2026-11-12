from harmos.v1 import types_pb2 as _types_pb2
from harmos.v1 import storage_types_pb2 as _storage_types_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
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

class StorageCapabilitiesRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class StorageCapabilitiesResponse(_message.Message):
    __slots__ = ("capabilities", "mutations", "file_batch_bytes")
    CAPABILITIES_FIELD_NUMBER: _ClassVar[int]
    MUTATIONS_FIELD_NUMBER: _ClassVar[int]
    FILE_BATCH_BYTES_FIELD_NUMBER: _ClassVar[int]
    capabilities: _storage_types_pb2.StorageCapabilities
    mutations: _storage_types_pb2.MutationWindow
    file_batch_bytes: int
    def __init__(self, capabilities: _Optional[_Union[_storage_types_pb2.StorageCapabilities, _Mapping]] = ..., mutations: _Optional[_Union[_storage_types_pb2.MutationWindow, _Mapping]] = ..., file_batch_bytes: _Optional[int] = ...) -> None: ...

class StorageCreateSnapshotRequest(_message.Message):
    __slots__ = ("at",)
    AT_FIELD_NUMBER: _ClassVar[int]
    at: _types_pb2.ReadCut
    def __init__(self, at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageCreateSnapshotResponse(_message.Message):
    __slots__ = ("snapshot", "resolved_at")
    SNAPSHOT_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    snapshot: _storage_types_pb2.SavedSnapshot
    resolved_at: _types_pb2.ReadCut
    def __init__(self, snapshot: _Optional[_Union[_storage_types_pb2.SavedSnapshot, _Mapping]] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageDeleteSnapshotRequest(_message.Message):
    __slots__ = ("snapshot",)
    SNAPSHOT_FIELD_NUMBER: _ClassVar[int]
    snapshot: _storage_types_pb2.SavedSnapshot
    def __init__(self, snapshot: _Optional[_Union[_storage_types_pb2.SavedSnapshot, _Mapping]] = ...) -> None: ...

class StorageDeleteSnapshotResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class StorageListColumnRowsRequest(_message.Message):
    __slots__ = ("snapshot", "read")
    SNAPSHOT_FIELD_NUMBER: _ClassVar[int]
    READ_FIELD_NUMBER: _ClassVar[int]
    snapshot: _storage_types_pb2.SavedSnapshot
    read: _storage_types_pb2.ArrowScan
    def __init__(self, snapshot: _Optional[_Union[_storage_types_pb2.SavedSnapshot, _Mapping]] = ..., read: _Optional[_Union[_storage_types_pb2.ArrowScan, _Mapping]] = ...) -> None: ...

class StorageListColumnRowsResponse(_message.Message):
    __slots__ = ("page", "resolved_at")
    PAGE_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    page: _storage_types_pb2.ArrowPage
    resolved_at: _types_pb2.ReadCut
    def __init__(self, page: _Optional[_Union[_storage_types_pb2.ArrowPage, _Mapping]] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageListIndexEntriesRequest(_message.Message):
    __slots__ = ("snapshot", "read", "source")
    SNAPSHOT_FIELD_NUMBER: _ClassVar[int]
    READ_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    snapshot: _storage_types_pb2.SavedSnapshot
    read: _storage_types_pb2.IndexRead
    source: _storage_types_pb2.ColumnSource
    def __init__(self, snapshot: _Optional[_Union[_storage_types_pb2.SavedSnapshot, _Mapping]] = ..., read: _Optional[_Union[_storage_types_pb2.IndexRead, _Mapping]] = ..., source: _Optional[_Union[_storage_types_pb2.ColumnSource, _Mapping]] = ...) -> None: ...

class StorageListIndexEntriesResponse(_message.Message):
    __slots__ = ("page", "resolved_at")
    PAGE_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    page: _storage_types_pb2.IndexPage
    resolved_at: _types_pb2.ReadCut
    def __init__(self, page: _Optional[_Union[_storage_types_pb2.IndexPage, _Mapping]] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageSetIndexDefinitionRequest(_message.Message):
    __slots__ = ("definition", "expected_revision", "create_if_absent", "rebuild", "work", "mutation")
    DEFINITION_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_REVISION_FIELD_NUMBER: _ClassVar[int]
    CREATE_IF_ABSENT_FIELD_NUMBER: _ClassVar[int]
    REBUILD_FIELD_NUMBER: _ClassVar[int]
    WORK_FIELD_NUMBER: _ClassVar[int]
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    definition: _storage_types_pb2.IndexDefinition
    expected_revision: bytes
    create_if_absent: bool
    rebuild: bool
    work: _storage_types_pb2.ReadLimits
    mutation: _storage_types_pb2.MutationId
    def __init__(self, definition: _Optional[_Union[_storage_types_pb2.IndexDefinition, _Mapping]] = ..., expected_revision: _Optional[bytes] = ..., create_if_absent: _Optional[bool] = ..., rebuild: _Optional[bool] = ..., work: _Optional[_Union[_storage_types_pb2.ReadLimits, _Mapping]] = ..., mutation: _Optional[_Union[_storage_types_pb2.MutationId, _Mapping]] = ...) -> None: ...

class StorageSetIndexDefinitionResponse(_message.Message):
    __slots__ = ("definition", "revision", "receipt")
    DEFINITION_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    definition: _storage_types_pb2.IndexDefinition
    revision: bytes
    receipt: _types_pb2.Receipt
    def __init__(self, definition: _Optional[_Union[_storage_types_pb2.IndexDefinition, _Mapping]] = ..., revision: _Optional[bytes] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageSetProjectionDefinitionRequest(_message.Message):
    __slots__ = ("definition", "expected_revision", "create_if_absent", "rebuild", "mutation")
    DEFINITION_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_REVISION_FIELD_NUMBER: _ClassVar[int]
    CREATE_IF_ABSENT_FIELD_NUMBER: _ClassVar[int]
    REBUILD_FIELD_NUMBER: _ClassVar[int]
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    definition: _storage_types_pb2.ProjectionDefinition
    expected_revision: bytes
    create_if_absent: bool
    rebuild: bool
    mutation: _storage_types_pb2.MutationId
    def __init__(self, definition: _Optional[_Union[_storage_types_pb2.ProjectionDefinition, _Mapping]] = ..., expected_revision: _Optional[bytes] = ..., create_if_absent: _Optional[bool] = ..., rebuild: _Optional[bool] = ..., mutation: _Optional[_Union[_storage_types_pb2.MutationId, _Mapping]] = ...) -> None: ...

class StorageSetProjectionDefinitionResponse(_message.Message):
    __slots__ = ("definition", "revision", "receipt")
    DEFINITION_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    definition: _storage_types_pb2.ProjectionDefinition
    revision: bytes
    receipt: _types_pb2.Receipt
    def __init__(self, definition: _Optional[_Union[_storage_types_pb2.ProjectionDefinition, _Mapping]] = ..., revision: _Optional[bytes] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageAdvanceProjectionRequest(_message.Message):
    __slots__ = ("snapshot", "name", "work", "source")
    SNAPSHOT_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    WORK_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    snapshot: _storage_types_pb2.SavedSnapshot
    name: str
    work: _storage_types_pb2.ReadLimits
    source: _storage_types_pb2.ColumnSource
    def __init__(self, snapshot: _Optional[_Union[_storage_types_pb2.SavedSnapshot, _Mapping]] = ..., name: _Optional[str] = ..., work: _Optional[_Union[_storage_types_pb2.ReadLimits, _Mapping]] = ..., source: _Optional[_Union[_storage_types_pb2.ColumnSource, _Mapping]] = ...) -> None: ...

class StorageAdvanceProjectionResponse(_message.Message):
    __slots__ = ("revision", "resolved_at")
    REVISION_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    revision: bytes
    resolved_at: _types_pb2.ReadCut
    def __init__(self, revision: _Optional[bytes] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageListProjectionRowsRequest(_message.Message):
    __slots__ = ("read", "source")
    READ_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    read: _storage_types_pb2.ProjectionRead
    source: _storage_types_pb2.ColumnSource
    def __init__(self, read: _Optional[_Union[_storage_types_pb2.ProjectionRead, _Mapping]] = ..., source: _Optional[_Union[_storage_types_pb2.ColumnSource, _Mapping]] = ...) -> None: ...

class StorageListProjectionRowsResponse(_message.Message):
    __slots__ = ("page", "resolved_at")
    PAGE_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    page: _storage_types_pb2.ProjectionPage
    resolved_at: _types_pb2.ReadCut
    def __init__(self, page: _Optional[_Union[_storage_types_pb2.ProjectionPage, _Mapping]] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageGetProjectionStatusRequest(_message.Message):
    __slots__ = ("name", "revision", "frontier", "source")
    NAME_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    FRONTIER_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    name: str
    revision: int
    frontier: _storage_types_pb2.StorageCursor
    source: _storage_types_pb2.ColumnSource
    def __init__(self, name: _Optional[str] = ..., revision: _Optional[int] = ..., frontier: _Optional[_Union[_storage_types_pb2.StorageCursor, _Mapping]] = ..., source: _Optional[_Union[_storage_types_pb2.ColumnSource, _Mapping]] = ...) -> None: ...

class StorageGetProjectionStatusResponse(_message.Message):
    __slots__ = ("status", "resolved_at")
    STATUS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    status: _storage_types_pb2.ProjectionStatus
    resolved_at: _types_pb2.ReadCut
    def __init__(self, status: _Optional[_Union[_storage_types_pb2.ProjectionStatus, _Mapping]] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageSetObjectRequest(_message.Message):
    __slots__ = ("mutation", "expected_revision", "create_if_absent", "schema", "value")
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_REVISION_FIELD_NUMBER: _ClassVar[int]
    CREATE_IF_ABSENT_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    mutation: _storage_types_pb2.MutationId
    expected_revision: bytes
    create_if_absent: bool
    schema: _storage_types_pb2.SchemaInput
    value: _storage_types_pb2.ObjectValue
    def __init__(self, mutation: _Optional[_Union[_storage_types_pb2.MutationId, _Mapping]] = ..., expected_revision: _Optional[bytes] = ..., create_if_absent: _Optional[bool] = ..., schema: _Optional[_Union[_storage_types_pb2.SchemaInput, _Mapping]] = ..., value: _Optional[_Union[_storage_types_pb2.ObjectValue, _Mapping]] = ...) -> None: ...

class StorageSetObjectResponse(_message.Message):
    __slots__ = ("value", "revision", "receipt")
    VALUE_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    value: _storage_types_pb2.ObjectValue
    revision: bytes
    receipt: _types_pb2.Receipt
    def __init__(self, value: _Optional[_Union[_storage_types_pb2.ObjectValue, _Mapping]] = ..., revision: _Optional[bytes] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageListRowsRequest(_message.Message):
    __slots__ = ("source", "at", "transforms", "page")
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    TRANSFORMS_FIELD_NUMBER: _ClassVar[int]
    PAGE_FIELD_NUMBER: _ClassVar[int]
    source: _storage_types_pb2.QuerySource
    at: _types_pb2.ReadCut
    transforms: _containers.RepeatedCompositeFieldContainer[_storage_types_pb2.Transform]
    page: _types_pb2.PageRequest
    def __init__(self, source: _Optional[_Union[_storage_types_pb2.QuerySource, _Mapping]] = ..., at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ..., transforms: _Optional[_Iterable[_Union[_storage_types_pb2.Transform, _Mapping]]] = ..., page: _Optional[_Union[_types_pb2.PageRequest, _Mapping]] = ...) -> None: ...

class StorageListRowsResponse(_message.Message):
    __slots__ = ("rows", "resolved_at", "next_page_token")
    ROWS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    NEXT_PAGE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    rows: _containers.RepeatedCompositeFieldContainer[_storage_types_pb2.QueryOutput]
    resolved_at: _types_pb2.ReadCut
    next_page_token: str
    def __init__(self, rows: _Optional[_Iterable[_Union[_storage_types_pb2.QueryOutput, _Mapping]]] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ..., next_page_token: _Optional[str] = ...) -> None: ...

class StorageWatchRowsRequest(_message.Message):
    __slots__ = ("source", "at")
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    source: _storage_types_pb2.QuerySource
    at: _types_pb2.ReadCut
    def __init__(self, source: _Optional[_Union[_storage_types_pb2.QuerySource, _Mapping]] = ..., at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageWatchRowsResponse(_message.Message):
    __slots__ = ("resolved_at", "boundary", "record", "sample", "object", "durable_progress", "resync_required")
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    BOUNDARY_FIELD_NUMBER: _ClassVar[int]
    RECORD_FIELD_NUMBER: _ClassVar[int]
    SAMPLE_FIELD_NUMBER: _ClassVar[int]
    OBJECT_FIELD_NUMBER: _ClassVar[int]
    DURABLE_PROGRESS_FIELD_NUMBER: _ClassVar[int]
    RESYNC_REQUIRED_FIELD_NUMBER: _ClassVar[int]
    resolved_at: _types_pb2.ReadCut
    boundary: _storage_types_pb2.Empty
    record: _storage_types_pb2.StoredRecord
    sample: _storage_types_pb2.StoredRow
    object: _storage_types_pb2.ObjectEntry
    durable_progress: _storage_types_pb2.Empty
    resync_required: _storage_types_pb2.StorageFailure
    def __init__(self, resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ..., boundary: _Optional[_Union[_storage_types_pb2.Empty, _Mapping]] = ..., record: _Optional[_Union[_storage_types_pb2.StoredRecord, _Mapping]] = ..., sample: _Optional[_Union[_storage_types_pb2.StoredRow, _Mapping]] = ..., object: _Optional[_Union[_storage_types_pb2.ObjectEntry, _Mapping]] = ..., durable_progress: _Optional[_Union[_storage_types_pb2.Empty, _Mapping]] = ..., resync_required: _Optional[_Union[_storage_types_pb2.StorageFailure, _Mapping]] = ...) -> None: ...

class StorageAppendRecordsRequest(_message.Message):
    __slots__ = ("mutation", "scope", "batch")
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    BATCH_FIELD_NUMBER: _ClassVar[int]
    mutation: _storage_types_pb2.MutationId
    scope: str
    batch: _storage_types_pb2.DataBatch
    def __init__(self, mutation: _Optional[_Union[_storage_types_pb2.MutationId, _Mapping]] = ..., scope: _Optional[str] = ..., batch: _Optional[_Union[_storage_types_pb2.DataBatch, _Mapping]] = ...) -> None: ...

class StorageAppendRecordsResponse(_message.Message):
    __slots__ = ("receipt",)
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    receipt: _types_pb2.Receipt
    def __init__(self, receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageAppendSamplesRequest(_message.Message):
    __slots__ = ("mutation", "scope", "batch")
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    BATCH_FIELD_NUMBER: _ClassVar[int]
    mutation: _storage_types_pb2.MutationId
    scope: str
    batch: _storage_types_pb2.DataBatch
    def __init__(self, mutation: _Optional[_Union[_storage_types_pb2.MutationId, _Mapping]] = ..., scope: _Optional[str] = ..., batch: _Optional[_Union[_storage_types_pb2.DataBatch, _Mapping]] = ...) -> None: ...

class StorageAppendSamplesResponse(_message.Message):
    __slots__ = ("receipt",)
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    receipt: _types_pb2.Receipt
    def __init__(self, receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageAppendObjectRevisionsRequest(_message.Message):
    __slots__ = ("mutation", "scope", "batch")
    MUTATION_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    BATCH_FIELD_NUMBER: _ClassVar[int]
    mutation: _storage_types_pb2.MutationId
    scope: str
    batch: _storage_types_pb2.DataBatch
    def __init__(self, mutation: _Optional[_Union[_storage_types_pb2.MutationId, _Mapping]] = ..., scope: _Optional[str] = ..., batch: _Optional[_Union[_storage_types_pb2.DataBatch, _Mapping]] = ...) -> None: ...

class StorageAppendObjectRevisionsResponse(_message.Message):
    __slots__ = ("receipt",)
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    receipt: _types_pb2.Receipt
    def __init__(self, receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageGetFileHashRequest(_message.Message):
    __slots__ = ("source", "range", "at")
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    RANGE_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    source: _storage_types_pb2.FileSource
    range: _storage_types_pb2.ByteRange
    at: _types_pb2.ReadCut
    def __init__(self, source: _Optional[_Union[_storage_types_pb2.FileSource, _Mapping]] = ..., range: _Optional[_Union[_storage_types_pb2.ByteRange, _Mapping]] = ..., at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageGetFileHashResponse(_message.Message):
    __slots__ = ("hash", "resolved_at")
    HASH_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    hash: _storage_types_pb2.FileHash
    resolved_at: _types_pb2.ReadCut
    def __init__(self, hash: _Optional[_Union[_storage_types_pb2.FileHash, _Mapping]] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageCaptureFileRequest(_message.Message):
    __slots__ = ("source", "range", "metadata", "consistency")
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    RANGE_FIELD_NUMBER: _ClassVar[int]
    METADATA_FIELD_NUMBER: _ClassVar[int]
    CONSISTENCY_FIELD_NUMBER: _ClassVar[int]
    source: _storage_types_pb2.FileSource
    range: _storage_types_pb2.ByteRange
    metadata: _storage_types_pb2.FileMetadata
    consistency: _storage_types_pb2.CaptureConsistency
    def __init__(self, source: _Optional[_Union[_storage_types_pb2.FileSource, _Mapping]] = ..., range: _Optional[_Union[_storage_types_pb2.ByteRange, _Mapping]] = ..., metadata: _Optional[_Union[_storage_types_pb2.FileMetadata, _Mapping]] = ..., consistency: _Optional[_Union[_storage_types_pb2.CaptureConsistency, str]] = ...) -> None: ...

class StorageCaptureFileResponse(_message.Message):
    __slots__ = ("file", "receipt")
    FILE_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    file: _storage_types_pb2.FileReference
    receipt: _types_pb2.Receipt
    def __init__(self, file: _Optional[_Union[_storage_types_pb2.FileReference, _Mapping]] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class StorageWatchFileBytesRequest(_message.Message):
    __slots__ = ("source", "start")
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    START_FIELD_NUMBER: _ClassVar[int]
    source: _storage_types_pb2.FileSource
    start: _storage_types_pb2.FollowStart
    def __init__(self, source: _Optional[_Union[_storage_types_pb2.FileSource, _Mapping]] = ..., start: _Optional[_Union[_storage_types_pb2.FollowStart, _Mapping]] = ...) -> None: ...

class StorageWatchFileBytesResponse(_message.Message):
    __slots__ = ("event",)
    EVENT_FIELD_NUMBER: _ClassVar[int]
    event: _storage_types_pb2.FileEvent
    def __init__(self, event: _Optional[_Union[_storage_types_pb2.FileEvent, _Mapping]] = ...) -> None: ...

class StorageWatchFileProductionRequest(_message.Message):
    __slots__ = ("file", "offset")
    FILE_FIELD_NUMBER: _ClassVar[int]
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    file: _storage_types_pb2.StagedFile
    offset: int
    def __init__(self, file: _Optional[_Union[_storage_types_pb2.StagedFile, _Mapping]] = ..., offset: _Optional[int] = ...) -> None: ...

class StorageWatchFileProductionResponse(_message.Message):
    __slots__ = ("data", "progress")
    DATA_FIELD_NUMBER: _ClassVar[int]
    PROGRESS_FIELD_NUMBER: _ClassVar[int]
    data: _storage_types_pb2.FileBatch
    progress: _storage_types_pb2.FileProgress
    def __init__(self, data: _Optional[_Union[_storage_types_pb2.FileBatch, _Mapping]] = ..., progress: _Optional[_Union[_storage_types_pb2.FileProgress, _Mapping]] = ...) -> None: ...

class StorageListDirectoryEntriesRequest(_message.Message):
    __slots__ = ("selection", "page")
    SELECTION_FIELD_NUMBER: _ClassVar[int]
    PAGE_FIELD_NUMBER: _ClassVar[int]
    selection: _storage_types_pb2.DirectorySelection
    page: _types_pb2.PageRequest
    def __init__(self, selection: _Optional[_Union[_storage_types_pb2.DirectorySelection, _Mapping]] = ..., page: _Optional[_Union[_types_pb2.PageRequest, _Mapping]] = ...) -> None: ...

class StorageListDirectoryEntriesResponse(_message.Message):
    __slots__ = ("entries", "next_page_token", "resolved_at")
    ENTRIES_FIELD_NUMBER: _ClassVar[int]
    NEXT_PAGE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    entries: _containers.RepeatedCompositeFieldContainer[_storage_types_pb2.DirectoryEntry]
    next_page_token: str
    resolved_at: _types_pb2.ReadCut
    def __init__(self, entries: _Optional[_Iterable[_Union[_storage_types_pb2.DirectoryEntry, _Mapping]]] = ..., next_page_token: _Optional[str] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageWatchDirectoryEntriesRequest(_message.Message):
    __slots__ = ("selection", "include_existing")
    SELECTION_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_EXISTING_FIELD_NUMBER: _ClassVar[int]
    selection: _storage_types_pb2.DirectorySelection
    include_existing: bool
    def __init__(self, selection: _Optional[_Union[_storage_types_pb2.DirectorySelection, _Mapping]] = ..., include_existing: _Optional[bool] = ...) -> None: ...

class StorageWatchDirectoryEntriesResponse(_message.Message):
    __slots__ = ("event",)
    EVENT_FIELD_NUMBER: _ClassVar[int]
    event: _storage_types_pb2.DirectoryEvent
    def __init__(self, event: _Optional[_Union[_storage_types_pb2.DirectoryEvent, _Mapping]] = ...) -> None: ...

class StorageGetIndexDefinitionRequest(_message.Message):
    __slots__ = ("name", "source")
    NAME_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    name: str
    source: _storage_types_pb2.ColumnSource
    def __init__(self, name: _Optional[str] = ..., source: _Optional[_Union[_storage_types_pb2.ColumnSource, _Mapping]] = ...) -> None: ...

class StorageGetIndexDefinitionResponse(_message.Message):
    __slots__ = ("definition", "revision", "resolved_at")
    DEFINITION_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    definition: _storage_types_pb2.IndexDefinition
    revision: bytes
    resolved_at: _types_pb2.ReadCut
    def __init__(self, definition: _Optional[_Union[_storage_types_pb2.IndexDefinition, _Mapping]] = ..., revision: _Optional[bytes] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageGetProjectionDefinitionRequest(_message.Message):
    __slots__ = ("name", "source")
    NAME_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    name: str
    source: _storage_types_pb2.ColumnSource
    def __init__(self, name: _Optional[str] = ..., source: _Optional[_Union[_storage_types_pb2.ColumnSource, _Mapping]] = ...) -> None: ...

class StorageGetProjectionDefinitionResponse(_message.Message):
    __slots__ = ("definition", "revision", "resolved_at")
    DEFINITION_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    definition: _storage_types_pb2.ProjectionDefinition
    revision: bytes
    resolved_at: _types_pb2.ReadCut
    def __init__(self, definition: _Optional[_Union[_storage_types_pb2.ProjectionDefinition, _Mapping]] = ..., revision: _Optional[bytes] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageGetObjectRequest(_message.Message):
    __slots__ = ("schema", "item", "at")
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    ITEM_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    schema: _storage_types_pb2.SchemaKey
    item: str
    at: _types_pb2.ReadCut
    def __init__(self, schema: _Optional[_Union[_storage_types_pb2.SchemaKey, _Mapping]] = ..., item: _Optional[str] = ..., at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class StorageGetObjectResponse(_message.Message):
    __slots__ = ("value", "revision", "exists", "resolved_at")
    VALUE_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    EXISTS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    value: _storage_types_pb2.ObjectValue
    revision: bytes
    exists: bool
    resolved_at: _types_pb2.ReadCut
    def __init__(self, value: _Optional[_Union[_storage_types_pb2.ObjectValue, _Mapping]] = ..., revision: _Optional[bytes] = ..., exists: _Optional[bool] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...
