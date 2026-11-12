import datetime

from harmos.v1 import plugin_pb2 as _plugin_pb2
from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf import duration_pb2 as _duration_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class CatalogKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CATALOG_KIND_OPERATIONS: _ClassVar[CatalogKind]
    CATALOG_KIND_SCHEMAS: _ClassVar[CatalogKind]
    CATALOG_KIND_STREAMS: _ClassVar[CatalogKind]
    CATALOG_KIND_OBJECTS: _ClassVar[CatalogKind]

class Visibility(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    VISIBILITY_UNSPECIFIED: _ClassVar[Visibility]
    VISIBILITY_PRIVATE: _ClassVar[Visibility]
    VISIBILITY_PUBLIC: _ClassVar[Visibility]
CATALOG_KIND_OPERATIONS: CatalogKind
CATALOG_KIND_SCHEMAS: CatalogKind
CATALOG_KIND_STREAMS: CatalogKind
CATALOG_KIND_OBJECTS: CatalogKind
VISIBILITY_UNSPECIFIED: Visibility
VISIBILITY_PRIVATE: Visibility
VISIBILITY_PUBLIC: Visibility

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

class InvocationsGetRequest(_message.Message):
    __slots__ = ("instance",)
    INSTANCE_FIELD_NUMBER: _ClassVar[int]
    instance: int
    def __init__(self, instance: _Optional[int] = ...) -> None: ...

class InvocationsCancelRequest(_message.Message):
    __slots__ = ("instance", "reason")
    INSTANCE_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    instance: int
    reason: str
    def __init__(self, instance: _Optional[int] = ..., reason: _Optional[str] = ...) -> None: ...

class InvocationsWatchRequest(_message.Message):
    __slots__ = ("instance",)
    INSTANCE_FIELD_NUMBER: _ClassVar[int]
    instance: int
    def __init__(self, instance: _Optional[int] = ...) -> None: ...

class InvocationsGetResponse(_message.Message):
    __slots__ = ("invocation",)
    INVOCATION_FIELD_NUMBER: _ClassVar[int]
    invocation: InstanceState
    def __init__(self, invocation: _Optional[_Union[InstanceState, _Mapping]] = ...) -> None: ...

class InvocationsCancelResponse(_message.Message):
    __slots__ = ("invocation",)
    INVOCATION_FIELD_NUMBER: _ClassVar[int]
    invocation: InstanceState
    def __init__(self, invocation: _Optional[_Union[InstanceState, _Mapping]] = ...) -> None: ...

class InvocationsWatchResponse(_message.Message):
    __slots__ = ("invocation",)
    INVOCATION_FIELD_NUMBER: _ClassVar[int]
    invocation: InstanceState
    def __init__(self, invocation: _Optional[_Union[InstanceState, _Mapping]] = ...) -> None: ...

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

class CatalogEntriesListRequest(_message.Message):
    __slots__ = ("kind", "schema", "page")
    KIND_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_FIELD_NUMBER: _ClassVar[int]
    PAGE_FIELD_NUMBER: _ClassVar[int]
    kind: CatalogKind
    schema: str
    page: _plugin_pb2.PageRequest
    def __init__(self, kind: _Optional[_Union[CatalogKind, str]] = ..., schema: _Optional[str] = ..., page: _Optional[_Union[_plugin_pb2.PageRequest, _Mapping]] = ...) -> None: ...

class CatalogEntriesListResponse(_message.Message):
    __slots__ = ("entries", "next_page_token", "operations", "resolved_at")
    ENTRIES_FIELD_NUMBER: _ClassVar[int]
    NEXT_PAGE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    OPERATIONS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    entries: bytes
    next_page_token: str
    operations: _containers.RepeatedCompositeFieldContainer[DiscoveredOperation]
    resolved_at: _plugin_pb2.ReadCut
    def __init__(self, entries: _Optional[bytes] = ..., next_page_token: _Optional[str] = ..., operations: _Optional[_Iterable[_Union[DiscoveredOperation, _Mapping]]] = ..., resolved_at: _Optional[_Union[_plugin_pb2.ReadCut, _Mapping]] = ...) -> None: ...

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

class OperationContract(_message.Message):
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
    policy: _plugin_pb2.OperationPolicy
    replay_safe: bool
    fingerprint: bytes
    protobuf: Protobuf
    grpc: Grpc
    def __init__(self, id: _Optional[str] = ..., description: _Optional[str] = ..., visibility: _Optional[_Union[Visibility, str]] = ..., policy: _Optional[_Union[_plugin_pb2.OperationPolicy, _Mapping]] = ..., replay_safe: _Optional[bool] = ..., fingerprint: _Optional[bytes] = ..., protobuf: _Optional[_Union[Protobuf, _Mapping]] = ..., grpc: _Optional[_Union[Grpc, _Mapping]] = ...) -> None: ...

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
    contract: OperationContract
    grants: _containers.RepeatedCompositeFieldContainer[Grant]
    conditional_access: bool
    grpc_available: bool
    def __init__(self, owner: _Optional[str] = ..., generation: _Optional[int] = ..., contract: _Optional[_Union[OperationContract, _Mapping]] = ..., grants: _Optional[_Iterable[_Union[Grant, _Mapping]]] = ..., conditional_access: _Optional[bool] = ..., grpc_available: _Optional[bool] = ...) -> None: ...

class SamplesAppendRequest(_message.Message):
    __slots__ = ("stream", "sequence", "published_at", "context", "payload", "following")
    STREAM_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    PUBLISHED_AT_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    FOLLOWING_FIELD_NUMBER: _ClassVar[int]
    stream: str
    sequence: int
    published_at: _timestamp_pb2.Timestamp
    context: str
    payload: bytes
    following: _containers.RepeatedCompositeFieldContainer[FollowingRow]
    def __init__(self, stream: _Optional[str] = ..., sequence: _Optional[int] = ..., published_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., context: _Optional[str] = ..., payload: _Optional[bytes] = ..., following: _Optional[_Iterable[_Union[FollowingRow, _Mapping]]] = ...) -> None: ...

class FollowingRow(_message.Message):
    __slots__ = ("payload", "published_delta")
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    PUBLISHED_DELTA_FIELD_NUMBER: _ClassVar[int]
    payload: bytes
    published_delta: _duration_pb2.Duration
    def __init__(self, payload: _Optional[bytes] = ..., published_delta: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ...) -> None: ...

class SamplesAppendResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...
