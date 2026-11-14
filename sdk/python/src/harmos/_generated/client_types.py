# @generated from protocol descriptors; do not edit.
from harmos._wire.host.v1.access.certificates_pb2 import CertificatesCreateResponse as CertificatesCreateResponse
from harmos._wire.host.v1.access.listener_pb2 import ListenerGetResponse as ListenerGetResponse
from harmos._wire.host.v1.access.listener_pb2 import ListenerSetResponse as ListenerSetResponse
from harmos._wire.host.v1.access.listener_pb2 import ListenerValue as ListenerValue
from harmos._wire.host.v1.access.pairings_pb2 import Pairing as Pairing
from harmos._wire.host.v1.access.pairings_pb2 import PairingsCreateResponse as PairingsCreateResponse
from harmos._wire.host.v1.access.pairings_pb2 import PairingsDeleteResponse as PairingsDeleteResponse
from harmos._wire.host.v1.access.pairings_pb2 import PairingsListResponse as PairingsListResponse
from harmos._wire.host.v1.invocations_pb2 import InvocationsCreateResponse as InvocationsCreateResponse
from harmos._wire.host.v1.plugins.settings_pb2 import SettingsSetResponse as SettingsSetResponse
from harmos._wire.host.v1.plugins_pb2 import PLUGIN_STATE_CONNECTING as PLUGIN_STATE_CONNECTING
from harmos._wire.host.v1.plugins_pb2 import PLUGIN_STATE_READY as PLUGIN_STATE_READY
from harmos._wire.host.v1.plugins_pb2 import PLUGIN_STATE_STOPPED as PLUGIN_STATE_STOPPED
from harmos._wire.host.v1.plugins_pb2 import PLUGIN_STATE_STOPPING as PLUGIN_STATE_STOPPING
from harmos._wire.host.v1.plugins_pb2 import PLUGIN_STATE_UNAVAILABLE as PLUGIN_STATE_UNAVAILABLE
from harmos._wire.host.v1.plugins_pb2 import PLUGIN_STATE_UNSPECIFIED as PLUGIN_STATE_UNSPECIFIED
from harmos._wire.host.v1.plugins_pb2 import PluginChange as PluginChange
from harmos._wire.host.v1.plugins_pb2 import PluginDescription as PluginDescription
from harmos._wire.host.v1.plugins_pb2 import PluginEntry as PluginEntry
from harmos._wire.host.v1.plugins_pb2 import PluginGuest as PluginGuest
from harmos._wire.host.v1.plugins_pb2 import PluginManaged as PluginManaged
from harmos._wire.host.v1.plugins_pb2 import PluginPlacement as PluginPlacement
from harmos._wire.host.v1.plugins_pb2 import PluginProcess as PluginProcess
from harmos._wire.host.v1.plugins_pb2 import PluginRemote as PluginRemote
from harmos._wire.host.v1.plugins_pb2 import PluginsCreateResponse as PluginsCreateResponse
from harmos._wire.host.v1.plugins_pb2 import PluginsDeleteResponse as PluginsDeleteResponse
from harmos._wire.host.v1.plugins_pb2 import PluginsGetResponse as PluginsGetResponse
from harmos._wire.host.v1.plugins_pb2 import PluginsListResponse as PluginsListResponse
from harmos._wire.host.v1.plugins_pb2 import PluginSnapshot as PluginSnapshot
from harmos._wire.host.v1.plugins_pb2 import PluginsReplaceResponse as PluginsReplaceResponse
from harmos._wire.host.v1.plugins_pb2 import PluginsRestartResponse as PluginsRestartResponse
from harmos._wire.host.v1.plugins_pb2 import PluginsSetResponse as PluginsSetResponse
from harmos._wire.host.v1.plugins_pb2 import PluginState as PluginState
from harmos._wire.host.v1.plugins_pb2 import PluginsWatchResponse as PluginsWatchResponse
from harmos._wire.host.v1.plugins_pb2 import PluginValue as PluginValue
from harmos._wire.host.v1.schema_pb2 import ADJACENT as ADJACENT
from harmos._wire.host.v1.schema_pb2 import BOOL as BOOL
from harmos._wire.host.v1.schema_pb2 import BYTES as BYTES
from harmos._wire.host.v1.schema_pb2 import EXTERNAL as EXTERNAL
from harmos._wire.host.v1.schema_pb2 import FLOAT32 as FLOAT32
from harmos._wire.host.v1.schema_pb2 import FLOAT64 as FLOAT64
from harmos._wire.host.v1.schema_pb2 import INT32 as INT32
from harmos._wire.host.v1.schema_pb2 import INT64 as INT64
from harmos._wire.host.v1.schema_pb2 import INTERNAL as INTERNAL
from harmos._wire.host.v1.schema_pb2 import MESSAGE as MESSAGE
from harmos._wire.host.v1.schema_pb2 import OBJECT as OBJECT
from harmos._wire.host.v1.schema_pb2 import OMIT_EMPTY as OMIT_EMPTY
from harmos._wire.host.v1.schema_pb2 import OMIT_NONE as OMIT_NONE
from harmos._wire.host.v1.schema_pb2 import OPTIONAL as OPTIONAL
from harmos._wire.host.v1.schema_pb2 import PAYLOAD_ENCODING_UNSPECIFIED as PAYLOAD_ENCODING_UNSPECIFIED
from harmos._wire.host.v1.schema_pb2 import POSTCARD_V1 as POSTCARD_V1
from harmos._wire.host.v1.schema_pb2 import REPEATED as REPEATED
from harmos._wire.host.v1.schema_pb2 import REQUIRED as REQUIRED
from harmos._wire.host.v1.schema_pb2 import STRING as STRING
from harmos._wire.host.v1.schema_pb2 import TUPLE as TUPLE
from harmos._wire.host.v1.schema_pb2 import UINT32 as UINT32
from harmos._wire.host.v1.schema_pb2 import UINT64 as UINT64
from harmos._wire.host.v1.schema_pb2 import UNIT as UNIT
from harmos._wire.host.v1.schema_pb2 import UNTAGGED as UNTAGGED
from harmos._wire.host.v1.schema_pb2 import VALUE as VALUE
from harmos._wire.host.v1.schema_pb2 import VARIANT as VARIANT
from harmos._wire.host.v1.schema_pb2 import PayloadEncoding as PayloadEncoding
from harmos._wire.host.v1.schema_pb2 import SchemaCardinality as SchemaCardinality
from harmos._wire.host.v1.schema_pb2 import SchemaDefinition as SchemaDefinition
from harmos._wire.host.v1.schema_pb2 import SchemaField as SchemaField
from harmos._wire.host.v1.schema_pb2 import SchemaForm as SchemaForm
from harmos._wire.host.v1.schema_pb2 import SchemaGraph as SchemaGraph
from harmos._wire.host.v1.schema_pb2 import SchemaKey as SchemaKey
from harmos._wire.host.v1.schema_pb2 import SchemaKind as SchemaKind
from harmos._wire.host.v1.schema_pb2 import SchemaOmission as SchemaOmission
from harmos._wire.host.v1.schema_pb2 import SchemaRepresentation as SchemaRepresentation
from harmos._wire.host.v1.schemas_pb2 import SchemaDescription as SchemaDescription
from harmos._wire.host.v1.schemas_pb2 import SchemasListResponse as SchemasListResponse
from harmos._wire.host.v1.status.diagnostics_pb2 import DiagnosticsGetResponse as DiagnosticsGetResponse
from harmos._wire.host.v1.status.logs_pb2 import Event as Event
from harmos._wire.host.v1.status.logs_pb2 import LogsListResponse as LogsListResponse
from harmos._wire.host.v1.status.logs_pb2 import LogsWatchResponse as LogsWatchResponse
from harmos._wire.host.v1.status.trace_events_pb2 import TraceEventsListResponse as TraceEventsListResponse
from harmos._wire.host.v1.status.trace_events_pb2 import TraceEventsWatchResponse as TraceEventsWatchResponse
from harmos._wire.host.v1.status_pb2 import COMPONENT_STATE_DEGRADED as COMPONENT_STATE_DEGRADED
from harmos._wire.host.v1.status_pb2 import COMPONENT_STATE_HEALTHY as COMPONENT_STATE_HEALTHY
from harmos._wire.host.v1.status_pb2 import COMPONENT_STATE_UNAVAILABLE as COMPONENT_STATE_UNAVAILABLE
from harmos._wire.host.v1.status_pb2 import COMPONENT_STATE_UNSPECIFIED as COMPONENT_STATE_UNSPECIFIED
from harmos._wire.host.v1.status_pb2 import ComponentState as ComponentState
from harmos._wire.host.v1.status_pb2 import HealthComponent as HealthComponent
from harmos._wire.host.v1.status_pb2 import HostIdentity as HostIdentity
from harmos._wire.host.v1.status_pb2 import HostStatus as HostStatus
from harmos._wire.host.v1.status_pb2 import StatusGetResponse as StatusGetResponse
from harmos._wire.host.v1.status_pb2 import StatusWatchResponse as StatusWatchResponse
from harmos._wire.host.v1.streams_pb2 import StreamIdentity as StreamIdentity
from harmos._wire.host.v1.streams_pb2 import StreamsListResponse as StreamsListResponse

CertificatesCreateResponse.__module__ = "harmos.client"
ListenerGetResponse.__module__ = "harmos.client"
ListenerSetResponse.__module__ = "harmos.client"
ListenerValue.__module__ = "harmos.client"
Pairing.__module__ = "harmos.client"
PairingsCreateResponse.__module__ = "harmos.client"
PairingsDeleteResponse.__module__ = "harmos.client"
PairingsListResponse.__module__ = "harmos.client"
InvocationsCreateResponse.__module__ = "harmos.client"
SettingsSetResponse.__module__ = "harmos.client"
PluginChange.__module__ = "harmos.client"
PluginDescription.__module__ = "harmos.client"
PluginEntry.__module__ = "harmos.client"
PluginGuest.__module__ = "harmos.client"
PluginManaged.__module__ = "harmos.client"
PluginPlacement.__module__ = "harmos.client"
PluginProcess.__module__ = "harmos.client"
PluginRemote.__module__ = "harmos.client"
PluginSnapshot.__module__ = "harmos.client"
PluginValue.__module__ = "harmos.client"
PluginsCreateResponse.__module__ = "harmos.client"
PluginsDeleteResponse.__module__ = "harmos.client"
PluginsGetResponse.__module__ = "harmos.client"
PluginsListResponse.__module__ = "harmos.client"
PluginsReplaceResponse.__module__ = "harmos.client"
PluginsRestartResponse.__module__ = "harmos.client"
PluginsSetResponse.__module__ = "harmos.client"
PluginsWatchResponse.__module__ = "harmos.client"
SchemaDefinition.__module__ = "harmos.client"
SchemaField.__module__ = "harmos.client"
SchemaGraph.__module__ = "harmos.client"
SchemaKey.__module__ = "harmos.client"
SchemaRepresentation.__module__ = "harmos.client"
SchemaDescription.__module__ = "harmos.client"
SchemasListResponse.__module__ = "harmos.client"
DiagnosticsGetResponse.__module__ = "harmos.client"
Event.__module__ = "harmos.client"
LogsListResponse.__module__ = "harmos.client"
LogsWatchResponse.__module__ = "harmos.client"
TraceEventsListResponse.__module__ = "harmos.client"
TraceEventsWatchResponse.__module__ = "harmos.client"
HealthComponent.__module__ = "harmos.client"
HostIdentity.__module__ = "harmos.client"
HostStatus.__module__ = "harmos.client"
StatusGetResponse.__module__ = "harmos.client"
StatusWatchResponse.__module__ = "harmos.client"
StreamIdentity.__module__ = "harmos.client"
StreamsListResponse.__module__ = "harmos.client"

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
