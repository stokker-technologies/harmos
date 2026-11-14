from harmos._wire.v1 import failure_pb2 as _failure_pb2
from harmos._wire.v1 import policy_pb2 as _policy_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

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
    failure: _failure_pb2.Failure
    def __init__(self, payload: _Optional[bytes] = ..., failure: _Optional[_Union[_failure_pb2.Failure, _Mapping]] = ...) -> None: ...

class ChildRequest(_message.Message):
    __slots__ = ("call", "operation", "payload", "fingerprint", "policy", "watch", "max_response_bytes", "background")
    CALL_FIELD_NUMBER: _ClassVar[int]
    OPERATION_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    POLICY_FIELD_NUMBER: _ClassVar[int]
    WATCH_FIELD_NUMBER: _ClassVar[int]
    MAX_RESPONSE_BYTES_FIELD_NUMBER: _ClassVar[int]
    BACKGROUND_FIELD_NUMBER: _ClassVar[int]
    call: int
    operation: str
    payload: bytes
    fingerprint: bytes
    policy: _policy_pb2.OperationPolicy
    watch: bool
    max_response_bytes: int
    background: bool
    def __init__(self, call: _Optional[int] = ..., operation: _Optional[str] = ..., payload: _Optional[bytes] = ..., fingerprint: _Optional[bytes] = ..., policy: _Optional[_Union[_policy_pb2.OperationPolicy, _Mapping]] = ..., watch: _Optional[bool] = ..., max_response_bytes: _Optional[int] = ..., background: _Optional[bool] = ...) -> None: ...

class ChildDemand(_message.Message):
    __slots__ = ("call",)
    CALL_FIELD_NUMBER: _ClassVar[int]
    call: int
    def __init__(self, call: _Optional[int] = ...) -> None: ...

class ChildCancel(_message.Message):
    __slots__ = ("call",)
    CALL_FIELD_NUMBER: _ClassVar[int]
    call: int
    def __init__(self, call: _Optional[int] = ...) -> None: ...

class ChildResponse(_message.Message):
    __slots__ = ("call", "payload", "failure", "item")
    CALL_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    FAILURE_FIELD_NUMBER: _ClassVar[int]
    ITEM_FIELD_NUMBER: _ClassVar[int]
    call: int
    payload: bytes
    failure: _failure_pb2.Failure
    item: bytes
    def __init__(self, call: _Optional[int] = ..., payload: _Optional[bytes] = ..., failure: _Optional[_Union[_failure_pb2.Failure, _Mapping]] = ..., item: _Optional[bytes] = ...) -> None: ...

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
