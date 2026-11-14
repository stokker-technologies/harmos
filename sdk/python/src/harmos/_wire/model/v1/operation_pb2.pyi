from harmos._wire.model.v1 import policy_pb2 as _policy_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Visibility(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    VISIBILITY_UNSPECIFIED: _ClassVar[Visibility]
    VISIBILITY_PRIVATE: _ClassVar[Visibility]
    VISIBILITY_PUBLIC: _ClassVar[Visibility]
    VISIBILITY_PROTECTED: _ClassVar[Visibility]
VISIBILITY_UNSPECIFIED: Visibility
VISIBILITY_PRIVATE: Visibility
VISIBILITY_PUBLIC: Visibility
VISIBILITY_PROTECTED: Visibility

class OperationContract(_message.Message):
    __slots__ = ("id", "request", "response", "visibility", "policy", "fingerprint", "description", "replay_safe")
    ID_FIELD_NUMBER: _ClassVar[int]
    REQUEST_FIELD_NUMBER: _ClassVar[int]
    RESPONSE_FIELD_NUMBER: _ClassVar[int]
    VISIBILITY_FIELD_NUMBER: _ClassVar[int]
    POLICY_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    REPLAY_SAFE_FIELD_NUMBER: _ClassVar[int]
    id: str
    request: str
    response: str
    visibility: Visibility
    policy: _policy_pb2.OperationPolicy
    fingerprint: bytes
    description: str
    replay_safe: bool
    def __init__(self, id: _Optional[str] = ..., request: _Optional[str] = ..., response: _Optional[str] = ..., visibility: _Optional[_Union[Visibility, str]] = ..., policy: _Optional[_Union[_policy_pb2.OperationPolicy, _Mapping]] = ..., fingerprint: _Optional[bytes] = ..., description: _Optional[str] = ..., replay_safe: _Optional[bool] = ...) -> None: ...

class Requirement(_message.Message):
    __slots__ = ("id", "fingerprint")
    ID_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    id: str
    fingerprint: bytes
    def __init__(self, id: _Optional[str] = ..., fingerprint: _Optional[bytes] = ...) -> None: ...

class Protobuf(_message.Message):
    __slots__ = ("descriptors", "request", "response", "input", "output")
    DESCRIPTORS_FIELD_NUMBER: _ClassVar[int]
    REQUEST_FIELD_NUMBER: _ClassVar[int]
    RESPONSE_FIELD_NUMBER: _ClassVar[int]
    INPUT_FIELD_NUMBER: _ClassVar[int]
    OUTPUT_FIELD_NUMBER: _ClassVar[int]
    descriptors: bytes
    request: str
    response: str
    input: str
    output: str
    def __init__(self, descriptors: _Optional[bytes] = ..., request: _Optional[str] = ..., response: _Optional[str] = ..., input: _Optional[str] = ..., output: _Optional[str] = ...) -> None: ...

class Grpc(_message.Message):
    __slots__ = ("package", "service", "method", "managed")
    PACKAGE_FIELD_NUMBER: _ClassVar[int]
    SERVICE_FIELD_NUMBER: _ClassVar[int]
    METHOD_FIELD_NUMBER: _ClassVar[int]
    MANAGED_FIELD_NUMBER: _ClassVar[int]
    package: str
    service: str
    method: str
    managed: bool
    def __init__(self, package: _Optional[str] = ..., service: _Optional[str] = ..., method: _Optional[str] = ..., managed: _Optional[bool] = ...) -> None: ...

class Grant(_message.Message):
    __slots__ = ("operation", "scope")
    OPERATION_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    operation: str
    scope: str
    def __init__(self, operation: _Optional[str] = ..., scope: _Optional[str] = ...) -> None: ...

class DiscoveredContract(_message.Message):
    __slots__ = ("id", "description", "visibility", "policy", "replay_safe", "fingerprint", "protobuf", "grpc")
    ID_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    VISIBILITY_FIELD_NUMBER: _ClassVar[int]
    POLICY_FIELD_NUMBER: _ClassVar[int]
    REPLAY_SAFE_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    PROTOBUF_FIELD_NUMBER: _ClassVar[int]
    GRPC_FIELD_NUMBER: _ClassVar[int]
    id: str
    description: str
    visibility: Visibility
    policy: _policy_pb2.OperationPolicy
    replay_safe: bool
    fingerprint: bytes
    protobuf: Protobuf
    grpc: Grpc
    def __init__(self, id: _Optional[str] = ..., description: _Optional[str] = ..., visibility: _Optional[_Union[Visibility, str]] = ..., policy: _Optional[_Union[_policy_pb2.OperationPolicy, _Mapping]] = ..., replay_safe: _Optional[bool] = ..., fingerprint: _Optional[bytes] = ..., protobuf: _Optional[_Union[Protobuf, _Mapping]] = ..., grpc: _Optional[_Union[Grpc, _Mapping]] = ...) -> None: ...

class DiscoveredOperation(_message.Message):
    __slots__ = ("owner", "generation", "contract", "grants", "conditional_access", "grpc_available")
    OWNER_FIELD_NUMBER: _ClassVar[int]
    GENERATION_FIELD_NUMBER: _ClassVar[int]
    CONTRACT_FIELD_NUMBER: _ClassVar[int]
    GRANTS_FIELD_NUMBER: _ClassVar[int]
    CONDITIONAL_ACCESS_FIELD_NUMBER: _ClassVar[int]
    GRPC_AVAILABLE_FIELD_NUMBER: _ClassVar[int]
    owner: str
    generation: int
    contract: DiscoveredContract
    grants: _containers.RepeatedCompositeFieldContainer[Grant]
    conditional_access: bool
    grpc_available: bool
    def __init__(self, owner: _Optional[str] = ..., generation: _Optional[int] = ..., contract: _Optional[_Union[DiscoveredContract, _Mapping]] = ..., grants: _Optional[_Iterable[_Union[Grant, _Mapping]]] = ..., conditional_access: _Optional[bool] = ..., grpc_available: _Optional[bool] = ...) -> None: ...
