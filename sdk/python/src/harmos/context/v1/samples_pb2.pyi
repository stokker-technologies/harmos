import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from harmos.v1 import types_pb2 as _types_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

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
    following: _containers.RepeatedCompositeFieldContainer[_types_pb2.FollowingRow]
    def __init__(self, stream: _Optional[str] = ..., sequence: _Optional[int] = ..., published_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., context: _Optional[str] = ..., payload: _Optional[bytes] = ..., following: _Optional[_Iterable[_Union[_types_pb2.FollowingRow, _Mapping]]] = ...) -> None: ...

class SamplesAppendResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...
