from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

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
