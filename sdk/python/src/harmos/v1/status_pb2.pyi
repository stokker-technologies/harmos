import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from harmos.v1 import types_pb2 as _types_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class ComponentState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    COMPONENT_STATE_UNSPECIFIED: _ClassVar[ComponentState]
    COMPONENT_STATE_HEALTHY: _ClassVar[ComponentState]
    COMPONENT_STATE_DEGRADED: _ClassVar[ComponentState]
    COMPONENT_STATE_UNAVAILABLE: _ClassVar[ComponentState]
COMPONENT_STATE_UNSPECIFIED: ComponentState
COMPONENT_STATE_HEALTHY: ComponentState
COMPONENT_STATE_DEGRADED: ComponentState
COMPONENT_STATE_UNAVAILABLE: ComponentState

class HostIdentity(_message.Message):
    __slots__ = ("application_name", "application_version", "library_version", "process_started_at", "host_id")
    APPLICATION_NAME_FIELD_NUMBER: _ClassVar[int]
    APPLICATION_VERSION_FIELD_NUMBER: _ClassVar[int]
    LIBRARY_VERSION_FIELD_NUMBER: _ClassVar[int]
    PROCESS_STARTED_AT_FIELD_NUMBER: _ClassVar[int]
    HOST_ID_FIELD_NUMBER: _ClassVar[int]
    application_name: str
    application_version: str
    library_version: str
    process_started_at: _timestamp_pb2.Timestamp
    host_id: str
    def __init__(self, application_name: _Optional[str] = ..., application_version: _Optional[str] = ..., library_version: _Optional[str] = ..., process_started_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., host_id: _Optional[str] = ...) -> None: ...

class HealthComponent(_message.Message):
    __slots__ = ("name", "state", "reason")
    NAME_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    name: str
    state: ComponentState
    reason: str
    def __init__(self, name: _Optional[str] = ..., state: _Optional[_Union[ComponentState, str]] = ..., reason: _Optional[str] = ...) -> None: ...

class HostStatus(_message.Message):
    __slots__ = ("identity", "available", "components")
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    AVAILABLE_FIELD_NUMBER: _ClassVar[int]
    COMPONENTS_FIELD_NUMBER: _ClassVar[int]
    identity: HostIdentity
    available: bool
    components: _containers.RepeatedCompositeFieldContainer[HealthComponent]
    def __init__(self, identity: _Optional[_Union[HostIdentity, _Mapping]] = ..., available: _Optional[bool] = ..., components: _Optional[_Iterable[_Union[HealthComponent, _Mapping]]] = ...) -> None: ...

class StatusGetRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class StatusGetResponse(_message.Message):
    __slots__ = ("status", "resolved_at")
    STATUS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    status: HostStatus
    resolved_at: _types_pb2.Position
    def __init__(self, status: _Optional[_Union[HostStatus, _Mapping]] = ..., resolved_at: _Optional[_Union[_types_pb2.Position, _Mapping]] = ...) -> None: ...

class StatusWatchRequest(_message.Message):
    __slots__ = ("at",)
    AT_FIELD_NUMBER: _ClassVar[int]
    at: _types_pb2.Position
    def __init__(self, at: _Optional[_Union[_types_pb2.Position, _Mapping]] = ...) -> None: ...

class StatusWatchResponse(_message.Message):
    __slots__ = ("status", "resolved_at")
    STATUS_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    status: HostStatus
    resolved_at: _types_pb2.Position
    def __init__(self, status: _Optional[_Union[HostStatus, _Mapping]] = ..., resolved_at: _Optional[_Union[_types_pb2.Position, _Mapping]] = ...) -> None: ...
