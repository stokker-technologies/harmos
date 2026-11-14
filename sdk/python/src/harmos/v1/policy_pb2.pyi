import datetime

from google.protobuf import duration_pb2 as _duration_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

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
