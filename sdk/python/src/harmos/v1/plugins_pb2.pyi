import datetime

from harmos.v1 import types_pb2 as _types_pb2
from harmos.v1 import storage_types_pb2 as _storage_types_pb2
from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class PluginState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PLUGIN_STATE_UNSPECIFIED: _ClassVar[PluginState]
    PLUGIN_STATE_CONNECTING: _ClassVar[PluginState]
    PLUGIN_STATE_READY: _ClassVar[PluginState]
    PLUGIN_STATE_UNAVAILABLE: _ClassVar[PluginState]
    PLUGIN_STATE_STOPPED: _ClassVar[PluginState]
    PLUGIN_STATE_STOPPING: _ClassVar[PluginState]
PLUGIN_STATE_UNSPECIFIED: PluginState
PLUGIN_STATE_CONNECTING: PluginState
PLUGIN_STATE_READY: PluginState
PLUGIN_STATE_UNAVAILABLE: PluginState
PLUGIN_STATE_STOPPED: PluginState
PLUGIN_STATE_STOPPING: PluginState

class PluginValue(_message.Message):
    __slots__ = ("enabled",)
    ENABLED_FIELD_NUMBER: _ClassVar[int]
    enabled: bool
    def __init__(self, enabled: _Optional[bool] = ...) -> None: ...

class PluginDescription(_message.Message):
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

class PluginRemote(_message.Message):
    __slots__ = ("url",)
    URL_FIELD_NUMBER: _ClassVar[int]
    url: str
    def __init__(self, url: _Optional[str] = ...) -> None: ...

class PluginManaged(_message.Message):
    __slots__ = ("content", "sha256")
    CONTENT_FIELD_NUMBER: _ClassVar[int]
    SHA256_FIELD_NUMBER: _ClassVar[int]
    content: _storage_types_pb2.FileReference
    sha256: bytes
    def __init__(self, content: _Optional[_Union[_storage_types_pb2.FileReference, _Mapping]] = ..., sha256: _Optional[bytes] = ...) -> None: ...

class PluginPlacement(_message.Message):
    __slots__ = ("managed", "remote")
    MANAGED_FIELD_NUMBER: _ClassVar[int]
    REMOTE_FIELD_NUMBER: _ClassVar[int]
    managed: PluginManaged
    remote: PluginRemote
    def __init__(self, managed: _Optional[_Union[PluginManaged, _Mapping]] = ..., remote: _Optional[_Union[PluginRemote, _Mapping]] = ...) -> None: ...

class PluginProcess(_message.Message):
    __slots__ = ("id", "executable_digest", "started_at")
    ID_FIELD_NUMBER: _ClassVar[int]
    EXECUTABLE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    STARTED_AT_FIELD_NUMBER: _ClassVar[int]
    id: int
    executable_digest: bytes
    started_at: _timestamp_pb2.Timestamp
    def __init__(self, id: _Optional[int] = ..., executable_digest: _Optional[bytes] = ..., started_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class PluginEntry(_message.Message):
    __slots__ = ("name", "placement", "value", "revision", "state", "failure", "description", "catalog_digest", "process", "activation")
    NAME_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    REVISION_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    FAILURE_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    CATALOG_DIGEST_FIELD_NUMBER: _ClassVar[int]
    PROCESS_FIELD_NUMBER: _ClassVar[int]
    ACTIVATION_FIELD_NUMBER: _ClassVar[int]
    name: str
    placement: PluginPlacement
    value: PluginValue
    revision: bytes
    state: PluginState
    failure: _types_pb2.Failure
    description: PluginDescription
    catalog_digest: bytes
    process: PluginProcess
    activation: int
    def __init__(self, name: _Optional[str] = ..., placement: _Optional[_Union[PluginPlacement, _Mapping]] = ..., value: _Optional[_Union[PluginValue, _Mapping]] = ..., revision: _Optional[bytes] = ..., state: _Optional[_Union[PluginState, str]] = ..., failure: _Optional[_Union[_types_pb2.Failure, _Mapping]] = ..., description: _Optional[_Union[PluginDescription, _Mapping]] = ..., catalog_digest: _Optional[bytes] = ..., process: _Optional[_Union[PluginProcess, _Mapping]] = ..., activation: _Optional[int] = ...) -> None: ...

class PluginsGetRequest(_message.Message):
    __slots__ = ("name", "at")
    NAME_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    name: str
    at: _types_pb2.ReadCut
    def __init__(self, name: _Optional[str] = ..., at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class PluginsGetResponse(_message.Message):
    __slots__ = ("entry", "resolved_at")
    ENTRY_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    entry: PluginEntry
    resolved_at: _types_pb2.ReadCut
    def __init__(self, entry: _Optional[_Union[PluginEntry, _Mapping]] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class PluginsListRequest(_message.Message):
    __slots__ = ("at", "page")
    AT_FIELD_NUMBER: _ClassVar[int]
    PAGE_FIELD_NUMBER: _ClassVar[int]
    at: _types_pb2.ReadCut
    page: _types_pb2.PageRequest
    def __init__(self, at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ..., page: _Optional[_Union[_types_pb2.PageRequest, _Mapping]] = ...) -> None: ...

class PluginsListResponse(_message.Message):
    __slots__ = ("entries", "resolved_at", "next_page_token")
    ENTRIES_FIELD_NUMBER: _ClassVar[int]
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    NEXT_PAGE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    entries: _containers.RepeatedCompositeFieldContainer[PluginEntry]
    resolved_at: _types_pb2.ReadCut
    next_page_token: str
    def __init__(self, entries: _Optional[_Iterable[_Union[PluginEntry, _Mapping]]] = ..., resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ..., next_page_token: _Optional[str] = ...) -> None: ...

class PluginsWatchRequest(_message.Message):
    __slots__ = ("name", "at")
    NAME_FIELD_NUMBER: _ClassVar[int]
    AT_FIELD_NUMBER: _ClassVar[int]
    name: str
    at: _types_pb2.ReadCut
    def __init__(self, name: _Optional[str] = ..., at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ...) -> None: ...

class PluginSnapshot(_message.Message):
    __slots__ = ("entries",)
    ENTRIES_FIELD_NUMBER: _ClassVar[int]
    entries: _containers.RepeatedCompositeFieldContainer[PluginEntry]
    def __init__(self, entries: _Optional[_Iterable[_Union[PluginEntry, _Mapping]]] = ...) -> None: ...

class PluginChange(_message.Message):
    __slots__ = ("entry", "deleted")
    ENTRY_FIELD_NUMBER: _ClassVar[int]
    DELETED_FIELD_NUMBER: _ClassVar[int]
    entry: PluginEntry
    deleted: str
    def __init__(self, entry: _Optional[_Union[PluginEntry, _Mapping]] = ..., deleted: _Optional[str] = ...) -> None: ...

class PluginsWatchResponse(_message.Message):
    __slots__ = ("resolved_at", "snapshot", "change", "resync_required")
    RESOLVED_AT_FIELD_NUMBER: _ClassVar[int]
    SNAPSHOT_FIELD_NUMBER: _ClassVar[int]
    CHANGE_FIELD_NUMBER: _ClassVar[int]
    RESYNC_REQUIRED_FIELD_NUMBER: _ClassVar[int]
    resolved_at: _types_pb2.ReadCut
    snapshot: PluginSnapshot
    change: PluginChange
    resync_required: str
    def __init__(self, resolved_at: _Optional[_Union[_types_pb2.ReadCut, _Mapping]] = ..., snapshot: _Optional[_Union[PluginSnapshot, _Mapping]] = ..., change: _Optional[_Union[PluginChange, _Mapping]] = ..., resync_required: _Optional[str] = ...) -> None: ...

class PluginsCreateRequest(_message.Message):
    __slots__ = ("request_id", "name", "placement", "value")
    REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    request_id: str
    name: str
    placement: PluginPlacement
    value: PluginValue
    def __init__(self, request_id: _Optional[str] = ..., name: _Optional[str] = ..., placement: _Optional[_Union[PluginPlacement, _Mapping]] = ..., value: _Optional[_Union[PluginValue, _Mapping]] = ...) -> None: ...

class PluginsCreateResponse(_message.Message):
    __slots__ = ("entry", "receipt")
    ENTRY_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    entry: PluginEntry
    receipt: _types_pb2.Receipt
    def __init__(self, entry: _Optional[_Union[PluginEntry, _Mapping]] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class PluginsSetRequest(_message.Message):
    __slots__ = ("name", "value", "expected_revision")
    NAME_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_REVISION_FIELD_NUMBER: _ClassVar[int]
    name: str
    value: PluginValue
    expected_revision: bytes
    def __init__(self, name: _Optional[str] = ..., value: _Optional[_Union[PluginValue, _Mapping]] = ..., expected_revision: _Optional[bytes] = ...) -> None: ...

class PluginsSetResponse(_message.Message):
    __slots__ = ("entry", "receipt")
    ENTRY_FIELD_NUMBER: _ClassVar[int]
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    entry: PluginEntry
    receipt: _types_pb2.Receipt
    def __init__(self, entry: _Optional[_Union[PluginEntry, _Mapping]] = ..., receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class PluginsDeleteRequest(_message.Message):
    __slots__ = ("name", "expected_revision")
    NAME_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_REVISION_FIELD_NUMBER: _ClassVar[int]
    name: str
    expected_revision: bytes
    def __init__(self, name: _Optional[str] = ..., expected_revision: _Optional[bytes] = ...) -> None: ...

class PluginsDeleteResponse(_message.Message):
    __slots__ = ("receipt",)
    RECEIPT_FIELD_NUMBER: _ClassVar[int]
    receipt: _types_pb2.Receipt
    def __init__(self, receipt: _Optional[_Union[_types_pb2.Receipt, _Mapping]] = ...) -> None: ...

class PluginsRestartRequest(_message.Message):
    __slots__ = ("name", "expected_revision")
    NAME_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_REVISION_FIELD_NUMBER: _ClassVar[int]
    name: str
    expected_revision: bytes
    def __init__(self, name: _Optional[str] = ..., expected_revision: _Optional[bytes] = ...) -> None: ...

class PluginsRestartResponse(_message.Message):
    __slots__ = ("entry",)
    ENTRY_FIELD_NUMBER: _ClassVar[int]
    entry: PluginEntry
    def __init__(self, entry: _Optional[_Union[PluginEntry, _Mapping]] = ...) -> None: ...
