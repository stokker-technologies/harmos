from harmos.v1 import types_pb2 as _types_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Pairing(_message.Message):
    __slots__ = ("id", "label")
    ID_FIELD_NUMBER: _ClassVar[int]
    LABEL_FIELD_NUMBER: _ClassVar[int]
    id: str
    label: str
    def __init__(self, id: _Optional[str] = ..., label: _Optional[str] = ...) -> None: ...

class PairingsCreateRequest(_message.Message):
    __slots__ = ("label",)
    LABEL_FIELD_NUMBER: _ClassVar[int]
    label: str
    def __init__(self, label: _Optional[str] = ...) -> None: ...

class PairingsCreateResponse(_message.Message):
    __slots__ = ("pairing", "endpoint", "pinned_certificate_pem", "credential")
    PAIRING_FIELD_NUMBER: _ClassVar[int]
    ENDPOINT_FIELD_NUMBER: _ClassVar[int]
    PINNED_CERTIFICATE_PEM_FIELD_NUMBER: _ClassVar[int]
    CREDENTIAL_FIELD_NUMBER: _ClassVar[int]
    pairing: Pairing
    endpoint: str
    pinned_certificate_pem: str
    credential: str
    def __init__(self, pairing: _Optional[_Union[Pairing, _Mapping]] = ..., endpoint: _Optional[str] = ..., pinned_certificate_pem: _Optional[str] = ..., credential: _Optional[str] = ...) -> None: ...

class PairingsListRequest(_message.Message):
    __slots__ = ("page", "at")
    PAGE_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    page: _types_pb2.PageRequest
    at: _types_pb2.Position
    def __init__(self, page: _Optional[_Union[_types_pb2.PageRequest, _Mapping]] = ..., at: _Optional[_Union[_types_pb2.Position, _Mapping]] = ...) -> None: ...

class PairingsListResponse(_message.Message):
    __slots__ = ("pairings", "resolved_at", "next_page_token")
    PAIRINGS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    NEXT_PAGE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    pairings: _containers.RepeatedCompositeFieldContainer[Pairing]
    resolved_at: _types_pb2.Position
    next_page_token: str
    def __init__(self, pairings: _Optional[_Iterable[_Union[Pairing, _Mapping]]] = ..., resolved_at: _Optional[_Union[_types_pb2.Position, _Mapping]] = ..., next_page_token: _Optional[str] = ...) -> None: ...

class PairingsDeleteRequest(_message.Message):
    __slots__ = ("name",)
    NAME_FIELD_NUMBER: _ClassVar[int]
    name: str
    def __init__(self, name: _Optional[str] = ...) -> None: ...

class PairingsDeleteResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...
