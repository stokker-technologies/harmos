# @generated from protocol descriptors; do not edit.
from harmos._wire.host.v1.access.certificates_pb2 import CertificatesCreateResponse
from harmos._wire.host.v1.access.listener_pb2 import ListenerGetResponse, ListenerSetResponse, ListenerValue
from harmos._wire.host.v1.access.pairings_pb2 import Pairing, PairingsCreateResponse, PairingsDeleteResponse, PairingsListResponse
from harmos._wire.host.v1.invocations_pb2 import InvocationsCreateResponse
from harmos._wire.host.v1.plugins.settings_pb2 import SettingsSetResponse
from harmos._wire.host.v1.plugins_pb2 import (
    PLUGIN_STATE_CONNECTING,
    PLUGIN_STATE_READY,
    PLUGIN_STATE_STOPPED,
    PLUGIN_STATE_STOPPING,
    PLUGIN_STATE_UNAVAILABLE,
    PLUGIN_STATE_UNSPECIFIED,
    PluginChange,
    PluginDescription,
    PluginEntry,
    PluginGuest,
    PluginManaged,
    PluginPlacement,
    PluginProcess,
    PluginRemote,
    PluginsCreateResponse,
    PluginsDeleteResponse,
    PluginsGetResponse,
    PluginsListResponse,
    PluginSnapshot,
    PluginsReplaceResponse,
    PluginsRestartResponse,
    PluginsSetResponse,
    PluginState,
    PluginsWatchResponse,
    PluginValue,
)
from harmos._wire.host.v1.schema_pb2 import (
    ADJACENT,
    BOOL,
    BYTES,
    EXTERNAL,
    FLOAT32,
    FLOAT64,
    INT32,
    INT64,
    INTERNAL,
    MESSAGE,
    OBJECT,
    OMIT_EMPTY,
    OMIT_NONE,
    OPTIONAL,
    PAYLOAD_ENCODING_UNSPECIFIED,
    POSTCARD_V1,
    REPEATED,
    REQUIRED,
    STRING,
    TUPLE,
    UINT32,
    UINT64,
    UNIT,
    UNTAGGED,
    VALUE,
    VARIANT,
    PayloadEncoding,
    SchemaCardinality,
    SchemaDefinition,
    SchemaField,
    SchemaForm,
    SchemaGraph,
    SchemaKey,
    SchemaKind,
    SchemaOmission,
    SchemaRepresentation,
)
from harmos._wire.host.v1.schemas_pb2 import SchemaDescription, SchemasListResponse
from harmos._wire.host.v1.status.diagnostics_pb2 import DiagnosticsGetResponse
from harmos._wire.host.v1.status.logs_pb2 import Event, LogsListResponse, LogsWatchResponse
from harmos._wire.host.v1.status.trace_events_pb2 import TraceEventsListResponse, TraceEventsWatchResponse
from harmos._wire.host.v1.status_pb2 import (
    COMPONENT_STATE_DEGRADED,
    COMPONENT_STATE_HEALTHY,
    COMPONENT_STATE_UNAVAILABLE,
    COMPONENT_STATE_UNSPECIFIED,
    ComponentState,
    HealthComponent,
    HostIdentity,
    HostStatus,
    StatusGetResponse,
    StatusWatchResponse,
)
from harmos._wire.host.v1.streams_pb2 import StreamIdentity, StreamsListResponse

CertificatesCreateResponse.__module__ = "harmos.client"
CertificatesCreateResponse.__qualname__ = "CertificatesCreateResponse"
ListenerGetResponse.__module__ = "harmos.client"
ListenerGetResponse.__qualname__ = "ListenerGetResponse"
ListenerSetResponse.__module__ = "harmos.client"
ListenerSetResponse.__qualname__ = "ListenerSetResponse"
ListenerValue.__module__ = "harmos.client"
ListenerValue.__qualname__ = "ListenerValue"
Pairing.__module__ = "harmos.client"
Pairing.__qualname__ = "Pairing"
PairingsCreateResponse.__module__ = "harmos.client"
PairingsCreateResponse.__qualname__ = "PairingsCreateResponse"
PairingsDeleteResponse.__module__ = "harmos.client"
PairingsDeleteResponse.__qualname__ = "PairingsDeleteResponse"
PairingsListResponse.__module__ = "harmos.client"
PairingsListResponse.__qualname__ = "PairingsListResponse"
InvocationsCreateResponse.__module__ = "harmos.client"
InvocationsCreateResponse.__qualname__ = "InvocationsCreateResponse"
SettingsSetResponse.__module__ = "harmos.client"
SettingsSetResponse.__qualname__ = "SettingsSetResponse"
PluginChange.__module__ = "harmos.client"
PluginChange.__qualname__ = "PluginChange"
PluginDescription.__module__ = "harmos.client"
PluginDescription.__qualname__ = "PluginDescription"
PluginEntry.__module__ = "harmos.client"
PluginEntry.__qualname__ = "PluginEntry"
PluginGuest.__module__ = "harmos.client"
PluginGuest.__qualname__ = "PluginGuest"
PluginManaged.__module__ = "harmos.client"
PluginManaged.__qualname__ = "PluginManaged"
PluginPlacement.__module__ = "harmos.client"
PluginPlacement.__qualname__ = "PluginPlacement"
PluginProcess.__module__ = "harmos.client"
PluginProcess.__qualname__ = "PluginProcess"
PluginRemote.__module__ = "harmos.client"
PluginRemote.__qualname__ = "PluginRemote"
PluginSnapshot.__module__ = "harmos.client"
PluginSnapshot.__qualname__ = "PluginSnapshot"
PluginValue.__module__ = "harmos.client"
PluginValue.__qualname__ = "PluginValue"
PluginsCreateResponse.__module__ = "harmos.client"
PluginsCreateResponse.__qualname__ = "PluginsCreateResponse"
PluginsDeleteResponse.__module__ = "harmos.client"
PluginsDeleteResponse.__qualname__ = "PluginsDeleteResponse"
PluginsGetResponse.__module__ = "harmos.client"
PluginsGetResponse.__qualname__ = "PluginsGetResponse"
PluginsListResponse.__module__ = "harmos.client"
PluginsListResponse.__qualname__ = "PluginsListResponse"
PluginsReplaceResponse.__module__ = "harmos.client"
PluginsReplaceResponse.__qualname__ = "PluginsReplaceResponse"
PluginsRestartResponse.__module__ = "harmos.client"
PluginsRestartResponse.__qualname__ = "PluginsRestartResponse"
PluginsSetResponse.__module__ = "harmos.client"
PluginsSetResponse.__qualname__ = "PluginsSetResponse"
PluginsWatchResponse.__module__ = "harmos.client"
PluginsWatchResponse.__qualname__ = "PluginsWatchResponse"
SchemaDefinition.__module__ = "harmos.client"
SchemaDefinition.__qualname__ = "SchemaDefinition"
SchemaField.__module__ = "harmos.client"
SchemaField.__qualname__ = "SchemaField"
SchemaGraph.__module__ = "harmos.client"
SchemaGraph.__qualname__ = "SchemaGraph"
SchemaKey.__module__ = "harmos.client"
SchemaKey.__qualname__ = "SchemaKey"
SchemaRepresentation.__module__ = "harmos.client"
SchemaRepresentation.__qualname__ = "SchemaRepresentation"
SchemaDescription.__module__ = "harmos.client"
SchemaDescription.__qualname__ = "SchemaDescription"
SchemasListResponse.__module__ = "harmos.client"
SchemasListResponse.__qualname__ = "SchemasListResponse"
DiagnosticsGetResponse.__module__ = "harmos.client"
DiagnosticsGetResponse.__qualname__ = "DiagnosticsGetResponse"
Event.__module__ = "harmos.client"
Event.__qualname__ = "Event"
LogsListResponse.__module__ = "harmos.client"
LogsListResponse.__qualname__ = "LogsListResponse"
LogsWatchResponse.__module__ = "harmos.client"
LogsWatchResponse.__qualname__ = "LogsWatchResponse"
TraceEventsListResponse.__module__ = "harmos.client"
TraceEventsListResponse.__qualname__ = "TraceEventsListResponse"
TraceEventsWatchResponse.__module__ = "harmos.client"
TraceEventsWatchResponse.__qualname__ = "TraceEventsWatchResponse"
HealthComponent.__module__ = "harmos.client"
HealthComponent.__qualname__ = "HealthComponent"
HostIdentity.__module__ = "harmos.client"
HostIdentity.__qualname__ = "HostIdentity"
HostStatus.__module__ = "harmos.client"
HostStatus.__qualname__ = "HostStatus"
StatusGetResponse.__module__ = "harmos.client"
StatusGetResponse.__qualname__ = "StatusGetResponse"
StatusWatchResponse.__module__ = "harmos.client"
StatusWatchResponse.__qualname__ = "StatusWatchResponse"
StreamIdentity.__module__ = "harmos.client"
StreamIdentity.__qualname__ = "StreamIdentity"
StreamsListResponse.__module__ = "harmos.client"
StreamsListResponse.__qualname__ = "StreamsListResponse"

__all__ = [
    "ADJACENT",
    "BOOL",
    "BYTES",
    "COMPONENT_STATE_DEGRADED",
    "COMPONENT_STATE_HEALTHY",
    "COMPONENT_STATE_UNAVAILABLE",
    "COMPONENT_STATE_UNSPECIFIED",
    "CertificatesCreateResponse",
    "ComponentState",
    "DiagnosticsGetResponse",
    "EXTERNAL",
    "Event",
    "FLOAT32",
    "FLOAT64",
    "HealthComponent",
    "HostIdentity",
    "HostStatus",
    "INT32",
    "INT64",
    "INTERNAL",
    "InvocationsCreateResponse",
    "ListenerGetResponse",
    "ListenerSetResponse",
    "ListenerValue",
    "LogsListResponse",
    "LogsWatchResponse",
    "MESSAGE",
    "OBJECT",
    "OMIT_EMPTY",
    "OMIT_NONE",
    "OPTIONAL",
    "PAYLOAD_ENCODING_UNSPECIFIED",
    "PLUGIN_STATE_CONNECTING",
    "PLUGIN_STATE_READY",
    "PLUGIN_STATE_STOPPED",
    "PLUGIN_STATE_STOPPING",
    "PLUGIN_STATE_UNAVAILABLE",
    "PLUGIN_STATE_UNSPECIFIED",
    "POSTCARD_V1",
    "Pairing",
    "PairingsCreateResponse",
    "PairingsDeleteResponse",
    "PairingsListResponse",
    "PayloadEncoding",
    "PluginChange",
    "PluginDescription",
    "PluginEntry",
    "PluginGuest",
    "PluginManaged",
    "PluginPlacement",
    "PluginProcess",
    "PluginRemote",
    "PluginSnapshot",
    "PluginState",
    "PluginValue",
    "PluginsCreateResponse",
    "PluginsDeleteResponse",
    "PluginsGetResponse",
    "PluginsListResponse",
    "PluginsReplaceResponse",
    "PluginsRestartResponse",
    "PluginsSetResponse",
    "PluginsWatchResponse",
    "REPEATED",
    "REQUIRED",
    "STRING",
    "SchemaCardinality",
    "SchemaDefinition",
    "SchemaDescription",
    "SchemaField",
    "SchemaForm",
    "SchemaGraph",
    "SchemaKey",
    "SchemaKind",
    "SchemaOmission",
    "SchemaRepresentation",
    "SchemasListResponse",
    "SettingsSetResponse",
    "StatusGetResponse",
    "StatusWatchResponse",
    "StreamIdentity",
    "StreamsListResponse",
    "TUPLE",
    "TraceEventsListResponse",
    "TraceEventsWatchResponse",
    "UINT32",
    "UINT64",
    "UNIT",
    "UNTAGGED",
    "VALUE",
    "VARIANT",
]
