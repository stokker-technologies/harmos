import datetime

from google.protobuf import duration_pb2 as _duration_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class SettingsDeclarations(_message.Message):
    __slots__ = ("values",)
    VALUES_FIELD_NUMBER: _ClassVar[int]
    values: _containers.RepeatedCompositeFieldContainer[SettingDeclaration]
    def __init__(self, values: _Optional[_Iterable[_Union[SettingDeclaration, _Mapping]]] = ...) -> None: ...

class SettingDeclaration(_message.Message):
    __slots__ = ("name", "title", "description", "default")
    NAME_FIELD_NUMBER: _ClassVar[int]
    TITLE_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    DEFAULT_FIELD_NUMBER: _ClassVar[int]
    name: str
    title: str
    description: str
    default: SettingValue
    def __init__(self, name: _Optional[str] = ..., title: _Optional[str] = ..., description: _Optional[str] = ..., default: _Optional[_Union[SettingValue, _Mapping]] = ...) -> None: ...

class SettingValue(_message.Message):
    __slots__ = ("flag", "integer", "number", "text", "duration")
    FLAG_FIELD_NUMBER: _ClassVar[int]
    INTEGER_FIELD_NUMBER: _ClassVar[int]
    NUMBER_FIELD_NUMBER: _ClassVar[int]
    TEXT_FIELD_NUMBER: _ClassVar[int]
    DURATION_FIELD_NUMBER: _ClassVar[int]
    flag: bool
    integer: int
    number: float
    text: str
    duration: _duration_pb2.Duration
    def __init__(self, flag: _Optional[bool] = ..., integer: _Optional[int] = ..., number: _Optional[float] = ..., text: _Optional[str] = ..., duration: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ...) -> None: ...

class SettingValues(_message.Message):
    __slots__ = ("values",)
    class ValuesEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: SettingValue
        def __init__(self, key: _Optional[str] = ..., value: _Optional[_Union[SettingValue, _Mapping]] = ...) -> None: ...
    VALUES_FIELD_NUMBER: _ClassVar[int]
    values: _containers.MessageMap[str, SettingValue]
    def __init__(self, values: _Optional[_Mapping[str, SettingValue]] = ...) -> None: ...
