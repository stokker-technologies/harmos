import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf import duration_pb2 as _duration_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class TimeSleepRequest(_message.Message):
    __slots__ = ("duration",)
    DURATION_FIELD_NUMBER: _ClassVar[int]
    duration: _duration_pb2.Duration
    def __init__(self, duration: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ...) -> None: ...

class TimeSleepResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class TimeGetRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class TimeGetResponse(_message.Message):
    __slots__ = ("now",)
    NOW_FIELD_NUMBER: _ClassVar[int]
    now: _timestamp_pb2.Timestamp
    def __init__(self, now: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...
