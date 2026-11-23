# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Payloads Client Calls Return."""

# === Imports === #

# Local Imports
from ._generated.proto.host.v1 import invocations_pb2, plugins_pb2, schema_pb2, schemas_pb2, status_pb2, streams_pb2
from ._generated.proto.host.v1.access import certificates_pb2, listener_pb2, pairings_pb2
from ._generated.proto.host.v1.plugins import settings_pb2
from ._generated.proto.host.v1.status import diagnostics_pb2, logs_pb2, trace_events_pb2

# === Public Exports === #

CertificatesCreateResponse = certificates_pb2.CertificatesCreateResponse
ListenerGetResponse = listener_pb2.ListenerGetResponse
ListenerSetResponse = listener_pb2.ListenerSetResponse
ListenerValue = listener_pb2.ListenerValue
Pairing = pairings_pb2.Pairing
PairingsCreateResponse = pairings_pb2.PairingsCreateResponse
PairingsDeleteResponse = pairings_pb2.PairingsDeleteResponse
PairingsListResponse = pairings_pb2.PairingsListResponse
InvocationsCreateResponse = invocations_pb2.InvocationsCreateResponse
SettingsSetResponse = settings_pb2.SettingsSetResponse
PLUGIN_STATE_CONNECTING = plugins_pb2.PLUGIN_STATE_CONNECTING
PLUGIN_STATE_READY = plugins_pb2.PLUGIN_STATE_READY
PLUGIN_STATE_STOPPED = plugins_pb2.PLUGIN_STATE_STOPPED
PLUGIN_STATE_STOPPING = plugins_pb2.PLUGIN_STATE_STOPPING
PLUGIN_STATE_UNAVAILABLE = plugins_pb2.PLUGIN_STATE_UNAVAILABLE
PLUGIN_STATE_UNSPECIFIED = plugins_pb2.PLUGIN_STATE_UNSPECIFIED
PluginChange = plugins_pb2.PluginChange
PluginDescription = plugins_pb2.PluginDescription
PluginEntry = plugins_pb2.PluginEntry
PluginGuest = plugins_pb2.PluginGuest
PluginManaged = plugins_pb2.PluginManaged
PluginPlacement = plugins_pb2.PluginPlacement
PluginProcess = plugins_pb2.PluginProcess
PluginRemote = plugins_pb2.PluginRemote
PluginSnapshot = plugins_pb2.PluginSnapshot
PluginState = plugins_pb2.PluginState
PluginValue = plugins_pb2.PluginValue
PluginsCreateResponse = plugins_pb2.PluginsCreateResponse
PluginsDeleteResponse = plugins_pb2.PluginsDeleteResponse
PluginsGetResponse = plugins_pb2.PluginsGetResponse
PluginsListResponse = plugins_pb2.PluginsListResponse
PluginsReplaceResponse = plugins_pb2.PluginsReplaceResponse
PluginsRestartResponse = plugins_pb2.PluginsRestartResponse
PluginsSetResponse = plugins_pb2.PluginsSetResponse
PluginsWatchResponse = plugins_pb2.PluginsWatchResponse
ADJACENT = schema_pb2.ADJACENT
BOOL = schema_pb2.BOOL
BYTES = schema_pb2.BYTES
EXTERNAL = schema_pb2.EXTERNAL
FLOAT32 = schema_pb2.FLOAT32
FLOAT64 = schema_pb2.FLOAT64
INT32 = schema_pb2.INT32
INT64 = schema_pb2.INT64
INTERNAL = schema_pb2.INTERNAL
MESSAGE = schema_pb2.MESSAGE
OBJECT = schema_pb2.OBJECT
OMIT_EMPTY = schema_pb2.OMIT_EMPTY
OMIT_NONE = schema_pb2.OMIT_NONE
OPTIONAL = schema_pb2.OPTIONAL
PAYLOAD_ENCODING_UNSPECIFIED = schema_pb2.PAYLOAD_ENCODING_UNSPECIFIED
POSTCARD_V1 = schema_pb2.POSTCARD_V1
PayloadEncoding = schema_pb2.PayloadEncoding
REPEATED = schema_pb2.REPEATED
REQUIRED = schema_pb2.REQUIRED
STRING = schema_pb2.STRING
SchemaCardinality = schema_pb2.SchemaCardinality
SchemaDefinition = schema_pb2.SchemaDefinition
SchemaField = schema_pb2.SchemaField
SchemaForm = schema_pb2.SchemaForm
SchemaGraph = schema_pb2.SchemaGraph
SchemaKey = schema_pb2.SchemaKey
SchemaKind = schema_pb2.SchemaKind
SchemaOmission = schema_pb2.SchemaOmission
SchemaRepresentation = schema_pb2.SchemaRepresentation
TUPLE = schema_pb2.TUPLE
UINT32 = schema_pb2.UINT32
UINT64 = schema_pb2.UINT64
UNIT = schema_pb2.UNIT
UNTAGGED = schema_pb2.UNTAGGED
VALUE = schema_pb2.VALUE
VARIANT = schema_pb2.VARIANT
SchemaDescription = schemas_pb2.SchemaDescription
SchemasListResponse = schemas_pb2.SchemasListResponse
DiagnosticsGetResponse = diagnostics_pb2.DiagnosticsGetResponse
Event = logs_pb2.Event
LogsListResponse = logs_pb2.LogsListResponse
LogsWatchResponse = logs_pb2.LogsWatchResponse
TraceEventsListResponse = trace_events_pb2.TraceEventsListResponse
TraceEventsWatchResponse = trace_events_pb2.TraceEventsWatchResponse
COMPONENT_STATE_DEGRADED = status_pb2.COMPONENT_STATE_DEGRADED
COMPONENT_STATE_HEALTHY = status_pb2.COMPONENT_STATE_HEALTHY
COMPONENT_STATE_UNAVAILABLE = status_pb2.COMPONENT_STATE_UNAVAILABLE
COMPONENT_STATE_UNSPECIFIED = status_pb2.COMPONENT_STATE_UNSPECIFIED
ComponentState = status_pb2.ComponentState
HealthComponent = status_pb2.HealthComponent
HostIdentity = status_pb2.HostIdentity
HostStatus = status_pb2.HostStatus
StatusGetResponse = status_pb2.StatusGetResponse
StatusWatchResponse = status_pb2.StatusWatchResponse
StreamIdentity = streams_pb2.StreamIdentity
StreamsListResponse = streams_pb2.StreamsListResponse

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
