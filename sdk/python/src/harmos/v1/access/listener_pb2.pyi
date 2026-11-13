from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class ListenerValue(_message.Message):
    __slots__ = ("enabled", "endpoint", "certificate_names")
    ENABLED_FIELD_NUMBER: _ClassVar[int]
    ENDPOINT_FIELD_NUMBER: _ClassVar[int]
    CERTIFICATE_NAMES_FIELD_NUMBER: _ClassVar[int]
    enabled: bool
    endpoint: str
    certificate_names: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, enabled: _Optional[bool] = ..., endpoint: _Optional[str] = ..., certificate_names: _Optional[_Iterable[str]] = ...) -> None: ...

class ListenerGetRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListenerGetResponse(_message.Message):
    __slots__ = ("value", "revision", "bound_endpoint")
    VALUE_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    BOUND_ENDPOINT_FIELD_NUMBER: _ClassVar[int]
    value: ListenerValue
    revision: bytes
    bound_endpoint: str
    def __init__(self, value: _Optional[_Union[ListenerValue, _Mapping]] = ..., revision: _Optional[bytes] = ..., bound_endpoint: _Optional[str] = ...) -> None: ...

class ListenerSetRequest(_message.Message):
    __slots__ = ("value", "expected_revision")
    VALUE_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_REVISION_FIELD_NUMBER: _ClassVar[int]
    value: ListenerValue
    expected_revision: bytes
    def __init__(self, value: _Optional[_Union[ListenerValue, _Mapping]] = ..., expected_revision: _Optional[bytes] = ...) -> None: ...

class ListenerSetResponse(_message.Message):
    __slots__ = ("value", "revision", "bound_endpoint")
    VALUE_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    BOUND_ENDPOINT_FIELD_NUMBER: _ClassVar[int]
    value: ListenerValue
    revision: bytes
    bound_endpoint: str
    def __init__(self, value: _Optional[_Union[ListenerValue, _Mapping]] = ..., revision: _Optional[bytes] = ..., bound_endpoint: _Optional[str] = ...) -> None: ...
