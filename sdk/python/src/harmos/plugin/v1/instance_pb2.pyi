import datetime

from harmos.v1 import types_pb2 as _types_pb2
from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class InstanceGetRequest(_message.Message):
    __slots__ = ("revision", "host", "host_version")
    REVISION_FIELD_NUMBER: _ClassVar[int]
    HOST_FIELD_NUMBER: _ClassVar[int]
    HOST_VERSION_FIELD_NUMBER: _ClassVar[int]
    revision: int
    host: str
    host_version: str
    def __init__(self, revision: _Optional[int] = ..., host: _Optional[str] = ..., host_version: _Optional[str] = ...) -> None: ...

class InstanceGetResponse(_message.Message):
    __slots__ = ("id", "version", "started_at", "catalog", "revision", "sdk", "sdk_version")
    ID_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    STARTED_AT_FIELD_NUMBER: _ClassVar[int]
    CATALOG_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    SDK_FIELD_NUMBER: _ClassVar[int]
    SDK_VERSION_FIELD_NUMBER: _ClassVar[int]
    id: str
    version: str
    started_at: _timestamp_pb2.Timestamp
    catalog: _types_pb2.Catalog
    revision: int
    sdk: str
    sdk_version: str
    def __init__(self, id: _Optional[str] = ..., version: _Optional[str] = ..., started_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., catalog: _Optional[_Union[_types_pb2.Catalog, _Mapping]] = ..., revision: _Optional[int] = ..., sdk: _Optional[str] = ..., sdk_version: _Optional[str] = ...) -> None: ...

class InstanceStopRequest(_message.Message):
    __slots__ = ("reason",)
    REASON_FIELD_NUMBER: _ClassVar[int]
    reason: str
    def __init__(self, reason: _Optional[str] = ...) -> None: ...

class InstanceStopResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class InstanceInvokeRequest(_message.Message):
    __slots__ = ("request", "child", "cancel")
    REQUEST_FIELD_NUMBER: _ClassVar[int]
    CHILD_FIELD_NUMBER: _ClassVar[int]
    CANCEL_FIELD_NUMBER: _ClassVar[int]
    request: _types_pb2.Request
    child: _types_pb2.ChildResponse
    cancel: _types_pb2.Cancel
    def __init__(self, request: _Optional[_Union[_types_pb2.Request, _Mapping]] = ..., child: _Optional[_Union[_types_pb2.ChildResponse, _Mapping]] = ..., cancel: _Optional[_Union[_types_pb2.Cancel, _Mapping]] = ...) -> None: ...

class InstanceInvokeResponse(_message.Message):
    __slots__ = ("child", "child_cancel", "response", "child_demand")
    CHILD_FIELD_NUMBER: _ClassVar[int]
    CHILD_CANCEL_FIELD_NUMBER: _ClassVar[int]
    RESPONSE_FIELD_NUMBER: _ClassVar[int]
    CHILD_DEMAND_FIELD_NUMBER: _ClassVar[int]
    child: _types_pb2.ChildRequest
    child_cancel: _types_pb2.ChildCancel
    response: _types_pb2.Response
    child_demand: _types_pb2.ChildDemand
    def __init__(self, child: _Optional[_Union[_types_pb2.ChildRequest, _Mapping]] = ..., child_cancel: _Optional[_Union[_types_pb2.ChildCancel, _Mapping]] = ..., response: _Optional[_Union[_types_pb2.Response, _Mapping]] = ..., child_demand: _Optional[_Union[_types_pb2.ChildDemand, _Mapping]] = ...) -> None: ...

class InstanceSamplesRequest(_message.Message):
    __slots__ = ("open", "tick")
    OPEN_FIELD_NUMBER: _ClassVar[int]
    TICK_FIELD_NUMBER: _ClassVar[int]
    open: _types_pb2.SamplesSelection
    tick: _types_pb2.Tick
    def __init__(self, open: _Optional[_Union[_types_pb2.SamplesSelection, _Mapping]] = ..., tick: _Optional[_Union[_types_pb2.Tick, _Mapping]] = ...) -> None: ...

class InstanceSamplesResponse(_message.Message):
    __slots__ = ("samples",)
    SAMPLES_FIELD_NUMBER: _ClassVar[int]
    samples: _containers.RepeatedCompositeFieldContainer[_types_pb2.Sample]
    def __init__(self, samples: _Optional[_Iterable[_Union[_types_pb2.Sample, _Mapping]]] = ...) -> None: ...
