import datetime

from google.protobuf import descriptor_pb2 as _descriptor_pb2
from google.protobuf import duration_pb2 as _duration_pb2
from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Visibility(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    VISIBILITY_UNSPECIFIED: _ClassVar[Visibility]
    VISIBILITY_PRIVATE: _ClassVar[Visibility]
    VISIBILITY_PUBLIC: _ClassVar[Visibility]

class Classification(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CLASSIFICATION_UNSPECIFIED: _ClassVar[Classification]
    CLASSIFICATION_PERMANENT: _ClassVar[Classification]
    CLASSIFICATION_TRANSIENT: _ClassVar[Classification]
    CLASSIFICATION_CANCELLED: _ClassVar[Classification]
    CLASSIFICATION_TIMED_OUT: _ClassVar[Classification]
    CLASSIFICATION_REFUSED: _ClassVar[Classification]

class CatalogKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CATALOG_KIND_OPERATIONS: _ClassVar[CatalogKind]
    CATALOG_KIND_SCHEMAS: _ClassVar[CatalogKind]
    CATALOG_KIND_STREAMS: _ClassVar[CatalogKind]
    CATALOG_KIND_OBJECTS: _ClassVar[CatalogKind]
VISIBILITY_UNSPECIFIED: Visibility
VISIBILITY_PRIVATE: Visibility
VISIBILITY_PUBLIC: Visibility
CLASSIFICATION_UNSPECIFIED: Classification
CLASSIFICATION_PERMANENT: Classification
CLASSIFICATION_TRANSIENT: Classification
CLASSIFICATION_CANCELLED: Classification
CLASSIFICATION_TIMED_OUT: Classification
CLASSIFICATION_REFUSED: Classification
CATALOG_KIND_OPERATIONS: CatalogKind
CATALOG_KIND_SCHEMAS: CatalogKind
CATALOG_KIND_STREAMS: CatalogKind
CATALOG_KIND_OBJECTS: CatalogKind

class History(_message.Message):
    __slots__ = ("store", "epoch")
    STORE_FIELD_NUMBER: _ClassVar[int]
    EPOCH_FIELD_NUMBER: _ClassVar[int]
    store: str
    epoch: str
    def __init__(self, store: _Optional[str] = ..., epoch: _Optional[str] = ...) -> None: ...

class Position(_message.Message):
    __slots__ = ("history", "sequence")
    HISTORY_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    history: History
    sequence: int
    def __init__(self, history: _Optional[_Union[History, _Mapping]] = ..., sequence: _Optional[int] = ...) -> None: ...

class ResourceRevision(_message.Message):
    __slots__ = ("name", "revision")
    NAME_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    name: str
    revision: bytes
    def __init__(self, name: _Optional[str] = ..., revision: _Optional[bytes] = ...) -> None: ...

class ReadCut(_message.Message):
    __slots__ = ("positions", "revisions")
    POSITIONS_FIELD_NUMBER: _ClassVar[int]
    REVISIONS_FIELD_NUMBER: _ClassVar[int]
    positions: _containers.RepeatedCompositeFieldContainer[Position]
    revisions: _containers.RepeatedCompositeFieldContainer[ResourceRevision]
    def __init__(self, positions: _Optional[_Iterable[_Union[Position, _Mapping]]] = ..., revisions: _Optional[_Iterable[_Union[ResourceRevision, _Mapping]]] = ...) -> None: ...

class PageRequest(_message.Message):
    __slots__ = ("page_size", "max_bytes", "page_token")
    PAGE_SIZE_FIELD_NUMBER: _ClassVar[int]
    MAX_BYTES_FIELD_NUMBER: _ClassVar[int]
    PAGE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    page_size: int
    max_bytes: int
    page_token: str
    def __init__(self, page_size: _Optional[int] = ..., max_bytes: _Optional[int] = ..., page_token: _Optional[str] = ...) -> None: ...

class Receipt(_message.Message):
    __slots__ = ("request_id", "accepted_at", "durable_at", "completed")
    REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_AT_FIELD_NUMBER: _ClassVar[int]
    DURABLE_AT_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_FIELD_NUMBER: _ClassVar[int]
    request_id: str
    accepted_at: Position
    durable_at: Position
    completed: bool
    def __init__(self, request_id: _Optional[str] = ..., accepted_at: _Optional[_Union[Position, _Mapping]] = ..., durable_at: _Optional[_Union[Position, _Mapping]] = ..., completed: _Optional[bool] = ...) -> None: ...

class Catalog(_message.Message):
    __slots__ = ("operations", "requires", "streams", "sections", "digest", "types")
    OPERATIONS_FIELD_NUMBER: _ClassVar[int]
    REQUIRES_FIELD_NUMBER: _ClassVar[int]
    STREAMS_FIELD_NUMBER: _ClassVar[int]
    SECTIONS_FIELD_NUMBER: _ClassVar[int]
    DIGEST_FIELD_NUMBER: _ClassVar[int]
    TYPES_FIELD_NUMBER: _ClassVar[int]
    operations: _containers.RepeatedCompositeFieldContainer[OperationContract]
    requires: _containers.RepeatedCompositeFieldContainer[Requirement]
    streams: _containers.RepeatedCompositeFieldContainer[StreamDeclaration]
    sections: _containers.RepeatedCompositeFieldContainer[Section]
    digest: bytes
    types: _descriptor_pb2.FileDescriptorSet
    def __init__(self, operations: _Optional[_Iterable[_Union[OperationContract, _Mapping]]] = ..., requires: _Optional[_Iterable[_Union[Requirement, _Mapping]]] = ..., streams: _Optional[_Iterable[_Union[StreamDeclaration, _Mapping]]] = ..., sections: _Optional[_Iterable[_Union[Section, _Mapping]]] = ..., digest: _Optional[bytes] = ..., types: _Optional[_Union[_descriptor_pb2.FileDescriptorSet, _Mapping]] = ...) -> None: ...

class OperationContract(_message.Message):
    __slots__ = ("id", "request", "response", "visibility", "policy", "fingerprint", "description", "replay_safe")
    ID_FIELD_NUMBER: _ClassVar[int]
    REQUEST_FIELD_NUMBER: _ClassVar[int]
    RESPONSE_FIELD_NUMBER: _ClassVar[int]
    VISIBILITY_FIELD_NUMBER: _ClassVar[int]
    POLICY_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    REPLAY_SAFE_FIELD_NUMBER: _ClassVar[int]
    id: str
    request: str
    response: str
    visibility: Visibility
    policy: OperationPolicy
    fingerprint: bytes
    description: str
    replay_safe: bool
    def __init__(self, id: _Optional[str] = ..., request: _Optional[str] = ..., response: _Optional[str] = ..., visibility: _Optional[_Union[Visibility, str]] = ..., policy: _Optional[_Union[OperationPolicy, _Mapping]] = ..., fingerprint: _Optional[bytes] = ..., description: _Optional[str] = ..., replay_safe: _Optional[bool] = ...) -> None: ...

class OperationPolicy(_message.Message):
    __slots__ = ("timeout", "attempt_timeout", "cancellation_timeout", "retries")
    TIMEOUT_FIELD_NUMBER: _ClassVar[int]
    ATTEMPT_TIMEOUT_FIELD_NUMBER: _ClassVar[int]
    CANCELLATION_TIMEOUT_FIELD_NUMBER: _ClassVar[int]
    RETRIES_FIELD_NUMBER: _ClassVar[int]
    timeout: _duration_pb2.Duration
    attempt_timeout: _duration_pb2.Duration
    cancellation_timeout: _duration_pb2.Duration
    retries: RetryPolicy
    def __init__(self, timeout: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ..., attempt_timeout: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ..., cancellation_timeout: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ..., retries: _Optional[_Union[RetryPolicy, _Mapping]] = ...) -> None: ...

class RetryPolicy(_message.Message):
    __slots__ = ("total_attempts", "initial", "maximum", "deterministic_jitter")
    TOTAL_ATTEMPTS_FIELD_NUMBER: _ClassVar[int]
    INITIAL_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_FIELD_NUMBER: _ClassVar[int]
    DETERMINISTIC_JITTER_FIELD_NUMBER: _ClassVar[int]
    total_attempts: int
    initial: _duration_pb2.Duration
    maximum: _duration_pb2.Duration
    deterministic_jitter: bool
    def __init__(self, total_attempts: _Optional[int] = ..., initial: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ..., maximum: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ..., deterministic_jitter: _Optional[bool] = ...) -> None: ...

class Requirement(_message.Message):
    __slots__ = ("id", "fingerprint")
    ID_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    id: str
    fingerprint: bytes
    def __init__(self, id: _Optional[str] = ..., fingerprint: _Optional[bytes] = ...) -> None: ...

class StreamDeclaration(_message.Message):
    __slots__ = ("id", "row", "description", "interval", "interval_setting")
    ID_FIELD_NUMBER: _ClassVar[int]
    ROW_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    INTERVAL_FIELD_NUMBER: _ClassVar[int]
    INTERVAL_SETTING_FIELD_NUMBER: _ClassVar[int]
    id: str
    row: str
    description: str
    interval: _duration_pb2.Duration
    interval_setting: str
    def __init__(self, id: _Optional[str] = ..., row: _Optional[str] = ..., description: _Optional[str] = ..., interval: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ..., interval_setting: _Optional[str] = ...) -> None: ...

class Section(_message.Message):
    __slots__ = ("type", "payload")
    TYPE_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    type: str
    payload: bytes
    def __init__(self, type: _Optional[str] = ..., payload: _Optional[bytes] = ...) -> None: ...

class Request(_message.Message):
    __slots__ = ("operation", "payload", "invocation", "attempt", "scope")
    OPERATION_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    INVOCATION_FIELD_NUMBER: _ClassVar[int]
    ATTEMPT_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    operation: str
    payload: bytes
    invocation: str
    attempt: int
    scope: str
    def __init__(self, operation: _Optional[str] = ..., payload: _Optional[bytes] = ..., invocation: _Optional[str] = ..., attempt: _Optional[int] = ..., scope: _Optional[str] = ...) -> None: ...

class Cancel(_message.Message):
    __slots__ = ("reason",)
    REASON_FIELD_NUMBER: _ClassVar[int]
    reason: str
    def __init__(self, reason: _Optional[str] = ...) -> None: ...

class Response(_message.Message):
    __slots__ = ("payload", "failure")
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    FAILURE_FIELD_NUMBER: _ClassVar[int]
    payload: bytes
    failure: Failure
    def __init__(self, payload: _Optional[bytes] = ..., failure: _Optional[_Union[Failure, _Mapping]] = ...) -> None: ...

class ChildRequest(_message.Message):
    __slots__ = ("call", "operation", "payload", "fingerprint", "policy")
    CALL_FIELD_NUMBER: _ClassVar[int]
    OPERATION_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    POLICY_FIELD_NUMBER: _ClassVar[int]
    call: int
    operation: str
    payload: bytes
    fingerprint: bytes
    policy: OperationPolicy
    def __init__(self, call: _Optional[int] = ..., operation: _Optional[str] = ..., payload: _Optional[bytes] = ..., fingerprint: _Optional[bytes] = ..., policy: _Optional[_Union[OperationPolicy, _Mapping]] = ...) -> None: ...

class ChildCancel(_message.Message):
    __slots__ = ("call",)
    CALL_FIELD_NUMBER: _ClassVar[int]
    call: int
    def __init__(self, call: _Optional[int] = ...) -> None: ...

class ChildResponse(_message.Message):
    __slots__ = ("call", "payload", "failure")
    CALL_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    FAILURE_FIELD_NUMBER: _ClassVar[int]
    call: int
    payload: bytes
    failure: Failure
    def __init__(self, call: _Optional[int] = ..., payload: _Optional[bytes] = ..., failure: _Optional[_Union[Failure, _Mapping]] = ...) -> None: ...

class Failure(_message.Message):
    __slots__ = ("classification", "code", "message", "action", "operation", "plugin", "cause", "details", "diagnostics")
    class DetailsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    CLASSIFICATION_FIELD_NUMBER: _ClassVar[int]
    CODE_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    ACTION_FIELD_NUMBER: _ClassVar[int]
    OPERATION_FIELD_NUMBER: _ClassVar[int]
    PLUGIN_FIELD_NUMBER: _ClassVar[int]
    CAUSE_FIELD_NUMBER: _ClassVar[int]
    DETAILS_FIELD_NUMBER: _ClassVar[int]
    DIAGNOSTICS_FIELD_NUMBER: _ClassVar[int]
    classification: Classification
    code: str
    message: str
    action: str
    operation: str
    plugin: str
    cause: Failure
    details: _containers.ScalarMap[str, str]
    diagnostics: str
    def __init__(self, classification: _Optional[_Union[Classification, str]] = ..., code: _Optional[str] = ..., message: _Optional[str] = ..., action: _Optional[str] = ..., operation: _Optional[str] = ..., plugin: _Optional[str] = ..., cause: _Optional[_Union[Failure, _Mapping]] = ..., details: _Optional[_Mapping[str, str]] = ..., diagnostics: _Optional[str] = ...) -> None: ...

class Tick(_message.Message):
    __slots__ = ("at",)
    AT_FIELD_NUMBER: _ClassVar[int]
    at: _timestamp_pb2.Timestamp
    def __init__(self, at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class SamplesSelection(_message.Message):
    __slots__ = ("stream", "scope")
    STREAM_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    stream: str
    scope: str
    def __init__(self, stream: _Optional[str] = ..., scope: _Optional[str] = ...) -> None: ...

class Sample(_message.Message):
    __slots__ = ("at", "row", "sequence")
    AT_FIELD_NUMBER: _ClassVar[int]
    ROW_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    at: _timestamp_pb2.Timestamp
    row: bytes
    sequence: int
    def __init__(self, at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., row: _Optional[bytes] = ..., sequence: _Optional[int] = ...) -> None: ...

class AssignedOperation(_message.Message):
    __slots__ = ("operation", "fingerprint", "generation")
    OPERATION_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    GENERATION_FIELD_NUMBER: _ClassVar[int]
    operation: str
    fingerprint: bytes
    generation: int
    def __init__(self, operation: _Optional[str] = ..., fingerprint: _Optional[bytes] = ..., generation: _Optional[int] = ...) -> None: ...

class OperationAssignments(_message.Message):
    __slots__ = ("operations",)
    OPERATIONS_FIELD_NUMBER: _ClassVar[int]
    operations: _containers.RepeatedCompositeFieldContainer[AssignedOperation]
    def __init__(self, operations: _Optional[_Iterable[_Union[AssignedOperation, _Mapping]]] = ...) -> None: ...

class InstanceState(_message.Message):
    __slots__ = ("phase", "outcome", "cancellation_requested", "attempt")
    PHASE_FIELD_NUMBER: _ClassVar[int]
    OUTCOME_FIELD_NUMBER: _ClassVar[int]
    CANCELLATION_REQUESTED_FIELD_NUMBER: _ClassVar[int]
    ATTEMPT_FIELD_NUMBER: _ClassVar[int]
    phase: str
    outcome: str
    cancellation_requested: bool
    attempt: int
    def __init__(self, phase: _Optional[str] = ..., outcome: _Optional[str] = ..., cancellation_requested: _Optional[bool] = ..., attempt: _Optional[int] = ...) -> None: ...

class Protobuf(_message.Message):
    __slots__ = ("descriptors", "request", "response", "input", "output")
    DESCRIPTORS_FIELD_NUMBER: _ClassVar[int]
    REQUEST_FIELD_NUMBER: _ClassVar[int]
    RESPONSE_FIELD_NUMBER: _ClassVar[int]
    INPUT_FIELD_NUMBER: _ClassVar[int]
    OUTPUT_FIELD_NUMBER: _ClassVar[int]
    descriptors: bytes
    request: str
    response: str
    input: str
    output: str
    def __init__(self, descriptors: _Optional[bytes] = ..., request: _Optional[str] = ..., response: _Optional[str] = ..., input: _Optional[str] = ..., output: _Optional[str] = ...) -> None: ...

class Grpc(_message.Message):
    __slots__ = ("package", "service", "method", "managed")
    PACKAGE_FIELD_NUMBER: _ClassVar[int]
    SERVICE_FIELD_NUMBER: _ClassVar[int]
    METHOD_FIELD_NUMBER: _ClassVar[int]
    MANAGED_FIELD_NUMBER: _ClassVar[int]
    package: str
    service: str
    method: str
    managed: bool
    def __init__(self, package: _Optional[str] = ..., service: _Optional[str] = ..., method: _Optional[str] = ..., managed: _Optional[bool] = ...) -> None: ...

class Grant(_message.Message):
    __slots__ = ("operation", "scope")
    OPERATION_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    operation: str
    scope: str
    def __init__(self, operation: _Optional[str] = ..., scope: _Optional[str] = ...) -> None: ...

class DiscoveredContract(_message.Message):
    __slots__ = ("id", "description", "visibility", "policy", "replay_safe", "fingerprint", "protobuf", "grpc")
    ID_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    VISIBILITY_FIELD_NUMBER: _ClassVar[int]
    POLICY_FIELD_NUMBER: _ClassVar[int]
    REPLAY_SAFE_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    PROTOBUF_FIELD_NUMBER: _ClassVar[int]
    GRPC_FIELD_NUMBER: _ClassVar[int]
    id: str
    description: str
    visibility: Visibility
    policy: OperationPolicy
    replay_safe: bool
    fingerprint: bytes
    protobuf: Protobuf
    grpc: Grpc
    def __init__(self, id: _Optional[str] = ..., description: _Optional[str] = ..., visibility: _Optional[_Union[Visibility, str]] = ..., policy: _Optional[_Union[OperationPolicy, _Mapping]] = ..., replay_safe: _Optional[bool] = ..., fingerprint: _Optional[bytes] = ..., protobuf: _Optional[_Union[Protobuf, _Mapping]] = ..., grpc: _Optional[_Union[Grpc, _Mapping]] = ...) -> None: ...

class DiscoveredOperation(_message.Message):
    __slots__ = ("owner", "generation", "contract", "grants", "conditional_access", "grpc_available")
    OWNER_FIELD_NUMBER: _ClassVar[int]
    GENERATION_FIELD_NUMBER: _ClassVar[int]
    CONTRACT_FIELD_NUMBER: _ClassVar[int]
    GRANTS_FIELD_NUMBER: _ClassVar[int]
    CONDITIONAL_ACCESS_FIELD_NUMBER: _ClassVar[int]
    GRPC_AVAILABLE_FIELD_NUMBER: _ClassVar[int]
    owner: str
    generation: int
    contract: DiscoveredContract
    grants: _containers.RepeatedCompositeFieldContainer[Grant]
    conditional_access: bool
    grpc_available: bool
    def __init__(self, owner: _Optional[str] = ..., generation: _Optional[int] = ..., contract: _Optional[_Union[DiscoveredContract, _Mapping]] = ..., grants: _Optional[_Iterable[_Union[Grant, _Mapping]]] = ..., conditional_access: _Optional[bool] = ..., grpc_available: _Optional[bool] = ...) -> None: ...

class FollowingRow(_message.Message):
    __slots__ = ("payload", "published_delta")
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    PUBLISHED_DELTA_FIELD_NUMBER: _ClassVar[int]
    payload: bytes
    published_delta: _duration_pb2.Duration
    def __init__(self, payload: _Optional[bytes] = ..., published_delta: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ...) -> None: ...
