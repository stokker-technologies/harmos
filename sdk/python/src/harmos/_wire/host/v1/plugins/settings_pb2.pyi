from harmos._wire.model.v1 import callers_pb2 as _callers_pb2
from harmos._wire.model.v1 import position_pb2 as _position_pb2
from harmos._wire.model.v1 import receipt_pb2 as _receipt_pb2
from harmos._wire.model.v1 import setting_pb2 as _setting_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class SettingsListRequest(_message.Message):
    __slots__ = ("name", "at")
    NAME_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    name: str
    at: _position_pb2.ReadCut
    def __init__(self, name: _Optional[str] = ..., at: _Optional[_Union[_position_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class SettingsListResponse(_message.Message):
    __slots__ = ("declarations", "value", "revision", "resolved_at")
    DECLARATIONS_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    declarations: _containers.RepeatedCompositeFieldContainer[_setting_pb2.SettingDeclaration]
    value: _setting_pb2.SettingValues
    revision: bytes
    resolved_at: _position_pb2.ReadCut
    def __init__(self, declarations: _Optional[_Iterable[_Union[_setting_pb2.SettingDeclaration, _Mapping]]] = ..., value: _Optional[_Union[_setting_pb2.SettingValues, _Mapping]] = ..., revision: _Optional[bytes] = ..., resolved_at: _Optional[_Union[_position_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class SettingsSetRequest(_message.Message):
    __slots__ = ("name", "value", "expected_revision", "request_id")
    NAME_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_REVISION_FIELD_NUMBER: _ClassVar[int]
    REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    name: str
    value: _setting_pb2.SettingValues
    expected_revision: bytes
    request_id: str
    def __init__(self, name: _Optional[str] = ..., value: _Optional[_Union[_setting_pb2.SettingValues, _Mapping]] = ..., expected_revision: _Optional[bytes] = ..., request_id: _Optional[str] = ...) -> None: ...

class SettingsSetResponse(_message.Message):
    __slots__ = ("value", "revision", "receipt")
    VALUE_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    value: _setting_pb2.SettingValues
    revision: bytes
    receipt: _receipt_pb2.Receipt
    def __init__(self, value: _Optional[_Union[_setting_pb2.SettingValues, _Mapping]] = ..., revision: _Optional[bytes] = ..., receipt: _Optional[_Union[_receipt_pb2.Receipt, _Mapping]] = ...) -> None: ...
