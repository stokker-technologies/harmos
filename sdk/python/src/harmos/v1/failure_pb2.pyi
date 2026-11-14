from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Classification(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CLASSIFICATION_UNSPECIFIED: _ClassVar[Classification]
    CLASSIFICATION_PERMANENT: _ClassVar[Classification]
    CLASSIFICATION_TRANSIENT: _ClassVar[Classification]
    CLASSIFICATION_CANCELLED: _ClassVar[Classification]
    CLASSIFICATION_TIMED_OUT: _ClassVar[Classification]
    CLASSIFICATION_REFUSED: _ClassVar[Classification]
CLASSIFICATION_UNSPECIFIED: Classification
CLASSIFICATION_PERMANENT: Classification
CLASSIFICATION_TRANSIENT: Classification
CLASSIFICATION_CANCELLED: Classification
CLASSIFICATION_TIMED_OUT: Classification
CLASSIFICATION_REFUSED: Classification

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
