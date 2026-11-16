// @generated from protocol descriptors; do not edit.
#pragma once
#include "harmos/host/v1/access/certificates.pb.h"
#include "harmos/host/v1/access/listener.pb.h"
#include "harmos/host/v1/access/pairings.pb.h"
#include "harmos/host/v1/invocations.pb.h"
#include "harmos/host/v1/operations.pb.h"
#include "harmos/host/v1/plugins.pb.h"
#include "harmos/host/v1/plugins/settings.pb.h"
#include "harmos/host/v1/schema.pb.h"
#include "harmos/host/v1/schemas.pb.h"
#include "harmos/host/v1/status.pb.h"
#include "harmos/host/v1/status/diagnostics.pb.h"
#include "harmos/host/v1/status/logs.pb.h"
#include "harmos/host/v1/status/trace_events.pb.h"
#include "harmos/host/v1/streams.pb.h"
#include "harmos/host/v1/time.pb.h"
#include "harmos/model/v1/callers.pb.h"
#include "harmos/model/v1/description.pb.h"
#include "harmos/model/v1/failure.pb.h"
#include "harmos/model/v1/file.pb.h"
#include "harmos/model/v1/invocation.pb.h"
#include "harmos/model/v1/operation.pb.h"
#include "harmos/model/v1/page.pb.h"
#include "harmos/model/v1/policy.pb.h"
#include "harmos/model/v1/position.pb.h"
#include "harmos/model/v1/receipt.pb.h"
#include "harmos/model/v1/setting.pb.h"
#include "harmos/model/v1/stream.pb.h"
#include "harmos/model/v1/transfer.pb.h"
namespace harmos::detail {
inline constexpr char stream_fingerprint_prefix[] = "harmos.protobuf.stream.v1\0";
namespace payloads {
using ::harmos::model::v1::Callers;
inline constexpr auto CALLERS_UNSPECIFIED = static_cast<::harmos::model::v1::Callers>(0);
inline constexpr auto CALLERS_CLIENTS = static_cast<::harmos::model::v1::Callers>(1);
inline constexpr auto CALLERS_PLUGINS = static_cast<::harmos::model::v1::Callers>(2);
inline constexpr auto CALLERS_ALL = static_cast<::harmos::model::v1::Callers>(3);
using ::harmos::host::v1::InvocationsCancelResponse;
using ::harmos::host::v1::InvocationsGetResponse;
using ::harmos::host::v1::InvocationsWatchResponse;
using ::harmos::model::v1::DiscoveredContract;
using ::harmos::model::v1::DiscoveredOperation;
using ::harmos::model::v1::Grant;
using ::harmos::model::v1::Grpc;
using ::harmos::model::v1::History;
using ::harmos::model::v1::InstanceState;
using ::harmos::model::v1::OperationContract;
using ::harmos::model::v1::OperationPolicy;
using ::harmos::model::v1::Position;
using ::harmos::model::v1::Protobuf;
using ::harmos::model::v1::ReadCut;
using ::harmos::model::v1::Requirement;
using ::harmos::model::v1::ResourceRevision;
using ::harmos::model::v1::RetryPolicy;
using ::harmos::model::v1::Visibility;
inline constexpr auto VISIBILITY_UNSPECIFIED = static_cast<::harmos::model::v1::Visibility>(0);
inline constexpr auto VISIBILITY_PRIVATE = static_cast<::harmos::model::v1::Visibility>(1);
inline constexpr auto VISIBILITY_PUBLIC = static_cast<::harmos::model::v1::Visibility>(2);
inline constexpr auto VISIBILITY_PROTECTED = static_cast<::harmos::model::v1::Visibility>(3);
using ::harmos::host::v1::OperationsGetResponse;
using ::harmos::host::v1::OperationsListResponse;
using ::harmos::host::v1::plugins::SettingsListResponse;
using ::harmos::model::v1::Declarations;
using ::harmos::model::v1::Receipt;
using ::harmos::model::v1::Section;
using ::harmos::model::v1::SettingDeclaration;
using ::harmos::model::v1::SettingsDeclarations;
using ::harmos::model::v1::SettingValue;
using ::harmos::model::v1::SettingValues;
using ::harmos::model::v1::StreamDeclaration;
using FailureValue = ::harmos::model::v1::Failure;
using ::harmos::model::v1::Classification;
inline constexpr auto CLASSIFICATION_UNSPECIFIED = static_cast<::harmos::model::v1::Classification>(0);
inline constexpr auto CLASSIFICATION_PERMANENT = static_cast<::harmos::model::v1::Classification>(1);
inline constexpr auto CLASSIFICATION_TRANSIENT = static_cast<::harmos::model::v1::Classification>(2);
inline constexpr auto CLASSIFICATION_CANCELLED = static_cast<::harmos::model::v1::Classification>(3);
inline constexpr auto CLASSIFICATION_TIMED_OUT = static_cast<::harmos::model::v1::Classification>(4);
inline constexpr auto CLASSIFICATION_REFUSED = static_cast<::harmos::model::v1::Classification>(5);
using ::harmos::model::v1::File;
using ::harmos::model::v1::FileMetadata;
using ::harmos::model::v1::FileReference;
using ::harmos::model::v1::Upload;
using ::harmos::model::v1::UploadState;
inline constexpr auto UPLOAD_STATE_UNSPECIFIED = static_cast<::harmos::model::v1::UploadState>(0);
inline constexpr auto UPLOAD_STATE_PRODUCING = static_cast<::harmos::model::v1::UploadState>(1);
inline constexpr auto UPLOAD_STATE_FINISHED = static_cast<::harmos::model::v1::UploadState>(2);
inline constexpr auto UPLOAD_STATE_ABANDONED = static_cast<::harmos::model::v1::UploadState>(3);
using ::harmos::host::v1::TimeGetResponse;
using ::harmos::host::v1::TimeSleepResponse;
using ::harmos::model::v1::MessageSettings;
using ::harmos::model::v1::PageRequest;
using ::harmos::model::v1::StorageTransferSettings;
using ::harmos::model::v1::TransferFraming;
inline constexpr auto TRANSFER_FRAMING_UNSPECIFIED = static_cast<::harmos::model::v1::TransferFraming>(0);
inline constexpr auto TRANSFER_FRAMING_BYTES = static_cast<::harmos::model::v1::TransferFraming>(512);
namespace client {
using ::harmos::host::v1::InvocationsCreateResponse;
using ::harmos::host::v1::PluginChange;
using ::harmos::host::v1::PluginDescription;
using ::harmos::host::v1::PluginEntry;
using ::harmos::host::v1::PluginGuest;
using ::harmos::host::v1::PluginManaged;
using ::harmos::host::v1::PluginPlacement;
using ::harmos::host::v1::PluginProcess;
using ::harmos::host::v1::PluginRemote;
using ::harmos::host::v1::PluginsCreateResponse;
using ::harmos::host::v1::PluginsDeleteResponse;
using ::harmos::host::v1::PluginsGetResponse;
using ::harmos::host::v1::PluginsListResponse;
using ::harmos::host::v1::PluginSnapshot;
using ::harmos::host::v1::PluginsReplaceResponse;
using ::harmos::host::v1::PluginsRestartResponse;
using ::harmos::host::v1::PluginsSetResponse;
using ::harmos::host::v1::PluginState;
using ::harmos::host::v1::PluginsWatchResponse;
using ::harmos::host::v1::PluginValue;
using ::harmos::host::v1::access::CertificatesCreateResponse;
using ::harmos::host::v1::access::ListenerGetResponse;
using ::harmos::host::v1::access::ListenerSetResponse;
using ::harmos::host::v1::access::ListenerValue;
using ::harmos::host::v1::access::Pairing;
using ::harmos::host::v1::access::PairingsCreateResponse;
using ::harmos::host::v1::access::PairingsDeleteResponse;
using ::harmos::host::v1::access::PairingsListResponse;
using ::harmos::host::v1::plugins::SettingsSetResponse;
inline constexpr auto PLUGIN_STATE_UNSPECIFIED = static_cast<::harmos::host::v1::PluginState>(0);
inline constexpr auto PLUGIN_STATE_CONNECTING = static_cast<::harmos::host::v1::PluginState>(1);
inline constexpr auto PLUGIN_STATE_READY = static_cast<::harmos::host::v1::PluginState>(2);
inline constexpr auto PLUGIN_STATE_UNAVAILABLE = static_cast<::harmos::host::v1::PluginState>(3);
inline constexpr auto PLUGIN_STATE_STOPPED = static_cast<::harmos::host::v1::PluginState>(4);
inline constexpr auto PLUGIN_STATE_STOPPING = static_cast<::harmos::host::v1::PluginState>(5);
using ::harmos::host::v1::PayloadEncoding;
using ::harmos::host::v1::SchemaCardinality;
using ::harmos::host::v1::SchemaDefinition;
using ::harmos::host::v1::SchemaField;
using ::harmos::host::v1::SchemaForm;
using ::harmos::host::v1::SchemaGraph;
using ::harmos::host::v1::SchemaKey;
using ::harmos::host::v1::SchemaKind;
using ::harmos::host::v1::SchemaOmission;
using ::harmos::host::v1::SchemaRepresentation;
inline constexpr auto PAYLOAD_ENCODING_UNSPECIFIED = static_cast<::harmos::host::v1::PayloadEncoding>(0);
inline constexpr auto PAYLOAD_ENCODING_POSTCARD_V1 = static_cast<::harmos::host::v1::PayloadEncoding>(1);
inline constexpr auto SCHEMA_CARDINALITY_REQUIRED = static_cast<::harmos::host::v1::SchemaCardinality>(0);
inline constexpr auto SCHEMA_CARDINALITY_OPTIONAL = static_cast<::harmos::host::v1::SchemaCardinality>(1);
inline constexpr auto SCHEMA_CARDINALITY_REPEATED = static_cast<::harmos::host::v1::SchemaCardinality>(2);
inline constexpr auto SCHEMA_CARDINALITY_VARIANT = static_cast<::harmos::host::v1::SchemaCardinality>(3);
inline constexpr auto SCHEMA_KIND_BOOL = static_cast<::harmos::host::v1::SchemaKind>(0);
inline constexpr auto SCHEMA_KIND_UINT32 = static_cast<::harmos::host::v1::SchemaKind>(1);
inline constexpr auto SCHEMA_KIND_UINT64 = static_cast<::harmos::host::v1::SchemaKind>(2);
inline constexpr auto SCHEMA_KIND_INT32 = static_cast<::harmos::host::v1::SchemaKind>(3);
inline constexpr auto SCHEMA_KIND_INT64 = static_cast<::harmos::host::v1::SchemaKind>(4);
inline constexpr auto SCHEMA_KIND_FLOAT32 = static_cast<::harmos::host::v1::SchemaKind>(5);
inline constexpr auto SCHEMA_KIND_FLOAT64 = static_cast<::harmos::host::v1::SchemaKind>(6);
inline constexpr auto SCHEMA_KIND_STRING = static_cast<::harmos::host::v1::SchemaKind>(7);
inline constexpr auto SCHEMA_KIND_BYTES = static_cast<::harmos::host::v1::SchemaKind>(8);
inline constexpr auto SCHEMA_KIND_MESSAGE = static_cast<::harmos::host::v1::SchemaKind>(9);
inline constexpr auto SCHEMA_FORM_OBJECT = static_cast<::harmos::host::v1::SchemaForm>(0);
inline constexpr auto SCHEMA_FORM_UNIT = static_cast<::harmos::host::v1::SchemaForm>(1);
inline constexpr auto SCHEMA_FORM_VALUE = static_cast<::harmos::host::v1::SchemaForm>(2);
inline constexpr auto SCHEMA_FORM_TUPLE = static_cast<::harmos::host::v1::SchemaForm>(3);
inline constexpr auto SCHEMA_FORM_EXTERNAL = static_cast<::harmos::host::v1::SchemaForm>(4);
inline constexpr auto SCHEMA_FORM_INTERNAL = static_cast<::harmos::host::v1::SchemaForm>(5);
inline constexpr auto SCHEMA_FORM_ADJACENT = static_cast<::harmos::host::v1::SchemaForm>(6);
inline constexpr auto SCHEMA_FORM_UNTAGGED = static_cast<::harmos::host::v1::SchemaForm>(7);
inline constexpr auto SCHEMA_OMISSION_OMIT_NONE = static_cast<::harmos::host::v1::SchemaOmission>(0);
inline constexpr auto SCHEMA_OMISSION_OMIT_EMPTY = static_cast<::harmos::host::v1::SchemaOmission>(1);
using ::harmos::host::v1::ComponentState;
using ::harmos::host::v1::HealthComponent;
using ::harmos::host::v1::HostIdentity;
using ::harmos::host::v1::HostStatus;
using ::harmos::host::v1::SchemaDescription;
using ::harmos::host::v1::SchemasListResponse;
using ::harmos::host::v1::StatusGetResponse;
using ::harmos::host::v1::StatusWatchResponse;
using ::harmos::host::v1::status::DiagnosticsGetResponse;
using ::harmos::host::v1::status::Event;
using ::harmos::host::v1::status::LogsListResponse;
using ::harmos::host::v1::status::LogsWatchResponse;
using ::harmos::host::v1::status::TraceEventsListResponse;
using ::harmos::host::v1::status::TraceEventsWatchResponse;
inline constexpr auto COMPONENT_STATE_UNSPECIFIED = static_cast<::harmos::host::v1::ComponentState>(0);
inline constexpr auto COMPONENT_STATE_HEALTHY = static_cast<::harmos::host::v1::ComponentState>(1);
inline constexpr auto COMPONENT_STATE_DEGRADED = static_cast<::harmos::host::v1::ComponentState>(2);
inline constexpr auto COMPONENT_STATE_UNAVAILABLE = static_cast<::harmos::host::v1::ComponentState>(3);
using ::harmos::host::v1::StreamIdentity;
using ::harmos::host::v1::StreamsListResponse;
} // namespace client
namespace plugin {}
} // namespace payloads
} // namespace harmos::detail
