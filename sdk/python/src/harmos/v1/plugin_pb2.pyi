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
VISIBILITY_UNSPECIFIED: Visibility
VISIBILITY_PRIVATE: Visibility
VISIBILITY_PUBLIC: Visibility
CLASSIFICATION_UNSPECIFIED: Classification
CLASSIFICATION_PERMANENT: Classification
CLASSIFICATION_TRANSIENT: Classification
CLASSIFICATION_CANCELLED: Classification
CLASSIFICATION_TIMED_OUT: Classification
CLASSIFICATION_REFUSED: Classification

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

class DescribeRequest(_message.Message):
    __slots__ = ("revision", "host", "host_version")
    REVISION_FIELD_NUMBER: _ClassVar[int]
    HOST_FIELD_NUMBER: _ClassVar[int]
    HOST_VERSION_FIELD_NUMBER: _ClassVar[int]
    revision: int
    host: str
    host_version: str
    def __init__(self, revision: _Optional[int] = ..., host: _Optional[str] = ..., host_version: _Optional[str] = ...) -> None: ...

class Description(_message.Message):
    __slots__ = ("id", "version", "started_at", "catalog", "revision", "sdk", "sdk_version")
    ID_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    STARTED_AT_FIELD_NUMBER: _ClassVar[int]
    CATALOG_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    SDK_FIELD_NUMBER: _ClassVar[int]
    SDK_VERSION_FIELD_NUMBER: _ClassVar[int]
    id: str
    version: str
    started_at: _timestamp_pb2.Timestamp
    catalog: Catalog
    revision: int
    sdk: str
    sdk_version: str
    def __init__(self, id: _Optional[str] = ..., version: _Optional[str] = ..., started_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., catalog: _Optional[_Union[Catalog, _Mapping]] = ..., revision: _Optional[int] = ..., sdk: _Optional[str] = ..., sdk_version: _Optional[str] = ...) -> None: ...

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

class StopRequest(_message.Message):
    __slots__ = ("reason",)
    REASON_FIELD_NUMBER: _ClassVar[int]
    reason: str
    def __init__(self, reason: _Optional[str] = ...) -> None: ...

class StopResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class HostFrame(_message.Message):
    __slots__ = ("request", "child", "cancel")
    REQUEST_FIELD_NUMBER: _ClassVar[int]
    CHILD_FIELD_NUMBER: _ClassVar[int]
    CANCEL_FIELD_NUMBER: _ClassVar[int]
    request: Request
    child: ChildResponse
    cancel: Cancel
    def __init__(self, request: _Optional[_Union[Request, _Mapping]] = ..., child: _Optional[_Union[ChildResponse, _Mapping]] = ..., cancel: _Optional[_Union[Cancel, _Mapping]] = ...) -> None: ...

class PluginFrame(_message.Message):
    __slots__ = ("child", "child_cancel", "response")
    CHILD_FIELD_NUMBER: _ClassVar[int]
    CHILD_CANCEL_FIELD_NUMBER: _ClassVar[int]
    RESPONSE_FIELD_NUMBER: _ClassVar[int]
    child: ChildRequest
    child_cancel: ChildCancel
    response: Response
    def __init__(self, child: _Optional[_Union[ChildRequest, _Mapping]] = ..., child_cancel: _Optional[_Union[ChildCancel, _Mapping]] = ..., response: _Optional[_Union[Response, _Mapping]] = ...) -> None: ...

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

class PublishFrame(_message.Message):
    __slots__ = ("open", "tick")
    OPEN_FIELD_NUMBER: _ClassVar[int]
    TICK_FIELD_NUMBER: _ClassVar[int]
    open: PublishRequest
    tick: Tick
    def __init__(self, open: _Optional[_Union[PublishRequest, _Mapping]] = ..., tick: _Optional[_Union[Tick, _Mapping]] = ...) -> None: ...

class Tick(_message.Message):
    __slots__ = ("at",)
    AT_FIELD_NUMBER: _ClassVar[int]
    at: _timestamp_pb2.Timestamp
    def __init__(self, at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class PublishRequest(_message.Message):
    __slots__ = ("stream", "scope")
    STREAM_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    stream: str
    scope: str
    def __init__(self, stream: _Optional[str] = ..., scope: _Optional[str] = ...) -> None: ...

class Samples(_message.Message):
    __slots__ = ("samples",)
    SAMPLES_FIELD_NUMBER: _ClassVar[int]
    samples: _containers.RepeatedCompositeFieldContainer[Sample]
    def __init__(self, samples: _Optional[_Iterable[_Union[Sample, _Mapping]]] = ...) -> None: ...

class Sample(_message.Message):
    __slots__ = ("at", "row", "sequence")
    AT_FIELD_NUMBER: _ClassVar[int]
    ROW_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    at: _timestamp_pb2.Timestamp
    row: bytes
    sequence: int
    def __init__(self, at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., row: _Optional[bytes] = ..., sequence: _Optional[int] = ...) -> None: ...

class Sleep(_message.Message):
    __slots__ = ("duration",)
    DURATION_FIELD_NUMBER: _ClassVar[int]
    duration: _duration_pb2.Duration
    def __init__(self, duration: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ...) -> None: ...

class SleepResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class Now(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class Time(_message.Message):
    __slots__ = ("now",)
    NOW_FIELD_NUMBER: _ClassVar[int]
    now: _timestamp_pb2.Timestamp
    def __init__(self, now: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...
