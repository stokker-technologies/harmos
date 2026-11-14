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
using ClientCertificatesCreateRequest = ::harmos::host::v1::access::CertificatesCreateRequest;
using ClientListenerGetRequest = ::harmos::host::v1::access::ListenerGetRequest;
using ClientListenerSetRequest = ::harmos::host::v1::access::ListenerSetRequest;
using ClientPairingsCreateRequest = ::harmos::host::v1::access::PairingsCreateRequest;
using ClientPairingsListRequest = ::harmos::host::v1::access::PairingsListRequest;
using ClientPairingsDeleteRequest = ::harmos::host::v1::access::PairingsDeleteRequest;
using ClientInvocationsCreateRequest = ::harmos::host::v1::InvocationsCreateRequest;
using SharedInvocationsGetRequest = ::harmos::host::v1::InvocationsGetRequest;
using SharedInvocationsCancelRequest = ::harmos::host::v1::InvocationsCancelRequest;
using SharedInvocationsWatchRequest = ::harmos::host::v1::InvocationsWatchRequest;
using SharedOperationsListRequest = ::harmos::host::v1::OperationsListRequest;
using SharedOperationsGetRequest = ::harmos::host::v1::OperationsGetRequest;
using SharedSettingsListRequest = ::harmos::host::v1::plugins::SettingsListRequest;
using ClientSettingsSetRequest = ::harmos::host::v1::plugins::SettingsSetRequest;
using ClientPluginsGetRequest = ::harmos::host::v1::PluginsGetRequest;
using ClientPluginsListRequest = ::harmos::host::v1::PluginsListRequest;
using ClientPluginsWatchRequest = ::harmos::host::v1::PluginsWatchRequest;
using ClientPluginsCreateRequest = ::harmos::host::v1::PluginsCreateRequest;
using ClientPluginsSetRequest = ::harmos::host::v1::PluginsSetRequest;
using ClientPluginsDeleteRequest = ::harmos::host::v1::PluginsDeleteRequest;
using ClientPluginsRestartRequest = ::harmos::host::v1::PluginsRestartRequest;
using ClientPluginsReplaceRequest = ::harmos::host::v1::PluginsReplaceRequest;
using ClientSchemasListRequest = ::harmos::host::v1::SchemasListRequest;
using ClientDiagnosticsGetRequest = ::harmos::host::v1::status::DiagnosticsGetRequest;
using ClientLogsListRequest = ::harmos::host::v1::status::LogsListRequest;
using ClientLogsWatchRequest = ::harmos::host::v1::status::LogsWatchRequest;
using ClientTraceEventsListRequest = ::harmos::host::v1::status::TraceEventsListRequest;
using ClientTraceEventsWatchRequest = ::harmos::host::v1::status::TraceEventsWatchRequest;
using ClientStatusGetRequest = ::harmos::host::v1::StatusGetRequest;
using ClientStatusWatchRequest = ::harmos::host::v1::StatusWatchRequest;
using ClientStreamsListRequest = ::harmos::host::v1::StreamsListRequest;
using SharedTimeGetRequest = ::harmos::host::v1::TimeGetRequest;
using SharedTimeSleepRequest = ::harmos::host::v1::TimeSleepRequest;
using SharedCallers = ::harmos::model::v1::Callers;
inline constexpr auto SharedCALLERS_UNSPECIFIED = static_cast<::harmos::model::v1::Callers>(0);
inline constexpr auto SharedCALLERS_CLIENTS = static_cast<::harmos::model::v1::Callers>(1);
inline constexpr auto SharedCALLERS_PLUGINS = static_cast<::harmos::model::v1::Callers>(2);
inline constexpr auto SharedCALLERS_ALL = static_cast<::harmos::model::v1::Callers>(3);
using ClientCertificatesCreateResponse = ::harmos::host::v1::access::CertificatesCreateResponse;
using ClientListenerGetResponse = ::harmos::host::v1::access::ListenerGetResponse;
using ClientListenerSetResponse = ::harmos::host::v1::access::ListenerSetResponse;
using ClientListenerValue = ::harmos::host::v1::access::ListenerValue;
using SharedHistory = ::harmos::model::v1::History;
using SharedPosition = ::harmos::model::v1::Position;
using SharedResourceRevision = ::harmos::model::v1::ResourceRevision;
using SharedReadCut = ::harmos::model::v1::ReadCut;
using ClientPairingsCreateResponse = ::harmos::host::v1::access::PairingsCreateResponse;
using ClientPairingsListResponse = ::harmos::host::v1::access::PairingsListResponse;
using ClientPairingsDeleteResponse = ::harmos::host::v1::access::PairingsDeleteResponse;
using ClientPairing = ::harmos::host::v1::access::Pairing;
using SharedInstanceState = ::harmos::model::v1::InstanceState;
using ClientInvocationsCreateResponse = ::harmos::host::v1::InvocationsCreateResponse;
using SharedInvocationsGetResponse = ::harmos::host::v1::InvocationsGetResponse;
using SharedInvocationsCancelResponse = ::harmos::host::v1::InvocationsCancelResponse;
using SharedInvocationsWatchResponse = ::harmos::host::v1::InvocationsWatchResponse;
using SharedOperationPolicy = ::harmos::model::v1::OperationPolicy;
using SharedRetryPolicy = ::harmos::model::v1::RetryPolicy;
using SharedOperationContract = ::harmos::model::v1::OperationContract;
using SharedRequirement = ::harmos::model::v1::Requirement;
using SharedProtobuf = ::harmos::model::v1::Protobuf;
using SharedGrpc = ::harmos::model::v1::Grpc;
using SharedGrant = ::harmos::model::v1::Grant;
using SharedDiscoveredContract = ::harmos::model::v1::DiscoveredContract;
using SharedDiscoveredOperation = ::harmos::model::v1::DiscoveredOperation;
using SharedVisibility = ::harmos::model::v1::Visibility;
inline constexpr auto SharedVISIBILITY_UNSPECIFIED = static_cast<::harmos::model::v1::Visibility>(0);
inline constexpr auto SharedVISIBILITY_PRIVATE = static_cast<::harmos::model::v1::Visibility>(1);
inline constexpr auto SharedVISIBILITY_PUBLIC = static_cast<::harmos::model::v1::Visibility>(2);
inline constexpr auto SharedVISIBILITY_PROTECTED = static_cast<::harmos::model::v1::Visibility>(3);
using SharedOperationsListResponse = ::harmos::host::v1::OperationsListResponse;
using SharedOperationsGetResponse = ::harmos::host::v1::OperationsGetResponse;
using SharedReceipt = ::harmos::model::v1::Receipt;
using SharedSettingsDeclarations = ::harmos::model::v1::SettingsDeclarations;
using SharedSettingDeclaration = ::harmos::model::v1::SettingDeclaration;
using SharedSettingValue = ::harmos::model::v1::SettingValue;
using SharedSettingValues = ::harmos::model::v1::SettingValues;
using SharedSettingsListResponse = ::harmos::host::v1::plugins::SettingsListResponse;
using ClientSettingsSetResponse = ::harmos::host::v1::plugins::SettingsSetResponse;
using SharedStreamDeclaration = ::harmos::model::v1::StreamDeclaration;
using SharedDeclarations = ::harmos::model::v1::Declarations;
using SharedSection = ::harmos::model::v1::Section;
using SharedFailureValue = ::harmos::model::v1::Failure;
using SharedClassification = ::harmos::model::v1::Classification;
inline constexpr auto SharedCLASSIFICATION_UNSPECIFIED = static_cast<::harmos::model::v1::Classification>(0);
inline constexpr auto SharedCLASSIFICATION_PERMANENT = static_cast<::harmos::model::v1::Classification>(1);
inline constexpr auto SharedCLASSIFICATION_TRANSIENT = static_cast<::harmos::model::v1::Classification>(2);
inline constexpr auto SharedCLASSIFICATION_CANCELLED = static_cast<::harmos::model::v1::Classification>(3);
inline constexpr auto SharedCLASSIFICATION_TIMED_OUT = static_cast<::harmos::model::v1::Classification>(4);
inline constexpr auto SharedCLASSIFICATION_REFUSED = static_cast<::harmos::model::v1::Classification>(5);
using SharedFileMetadata = ::harmos::model::v1::FileMetadata;
using SharedFileReference = ::harmos::model::v1::FileReference;
using SharedFile = ::harmos::model::v1::File;
using SharedUpload = ::harmos::model::v1::Upload;
using SharedUploadState = ::harmos::model::v1::UploadState;
inline constexpr auto SharedUPLOAD_STATE_UNSPECIFIED = static_cast<::harmos::model::v1::UploadState>(0);
inline constexpr auto SharedUPLOAD_STATE_PRODUCING = static_cast<::harmos::model::v1::UploadState>(1);
inline constexpr auto SharedUPLOAD_STATE_FINISHED = static_cast<::harmos::model::v1::UploadState>(2);
inline constexpr auto SharedUPLOAD_STATE_ABANDONED = static_cast<::harmos::model::v1::UploadState>(3);
using ClientPluginsGetResponse = ::harmos::host::v1::PluginsGetResponse;
using ClientPluginsListResponse = ::harmos::host::v1::PluginsListResponse;
using ClientPluginsWatchResponse = ::harmos::host::v1::PluginsWatchResponse;
using ClientPluginsCreateResponse = ::harmos::host::v1::PluginsCreateResponse;
using ClientPluginsSetResponse = ::harmos::host::v1::PluginsSetResponse;
using ClientPluginsDeleteResponse = ::harmos::host::v1::PluginsDeleteResponse;
using ClientPluginsRestartResponse = ::harmos::host::v1::PluginsRestartResponse;
using ClientPluginsReplaceResponse = ::harmos::host::v1::PluginsReplaceResponse;
using ClientPluginValue = ::harmos::host::v1::PluginValue;
using ClientPluginDescription = ::harmos::host::v1::PluginDescription;
using ClientPluginRemote = ::harmos::host::v1::PluginRemote;
using ClientPluginManaged = ::harmos::host::v1::PluginManaged;
using ClientPluginGuest = ::harmos::host::v1::PluginGuest;
using ClientPluginPlacement = ::harmos::host::v1::PluginPlacement;
using ClientPluginProcess = ::harmos::host::v1::PluginProcess;
using ClientPluginEntry = ::harmos::host::v1::PluginEntry;
using ClientPluginSnapshot = ::harmos::host::v1::PluginSnapshot;
using ClientPluginChange = ::harmos::host::v1::PluginChange;
using ClientPluginState = ::harmos::host::v1::PluginState;
inline constexpr auto ClientPLUGIN_STATE_UNSPECIFIED = static_cast<::harmos::host::v1::PluginState>(0);
inline constexpr auto ClientPLUGIN_STATE_CONNECTING = static_cast<::harmos::host::v1::PluginState>(1);
inline constexpr auto ClientPLUGIN_STATE_READY = static_cast<::harmos::host::v1::PluginState>(2);
inline constexpr auto ClientPLUGIN_STATE_UNAVAILABLE = static_cast<::harmos::host::v1::PluginState>(3);
inline constexpr auto ClientPLUGIN_STATE_STOPPED = static_cast<::harmos::host::v1::PluginState>(4);
inline constexpr auto ClientPLUGIN_STATE_STOPPING = static_cast<::harmos::host::v1::PluginState>(5);
using ClientSchemaKey = ::harmos::host::v1::SchemaKey;
using ClientSchemaField = ::harmos::host::v1::SchemaField;
using ClientSchemaDefinition = ::harmos::host::v1::SchemaDefinition;
using ClientSchemaRepresentation = ::harmos::host::v1::SchemaRepresentation;
using ClientSchemaGraph = ::harmos::host::v1::SchemaGraph;
using ClientPayloadEncoding = ::harmos::host::v1::PayloadEncoding;
using ClientSchemaCardinality = ::harmos::host::v1::SchemaCardinality;
using ClientSchemaKind = ::harmos::host::v1::SchemaKind;
using ClientSchemaForm = ::harmos::host::v1::SchemaForm;
using ClientSchemaOmission = ::harmos::host::v1::SchemaOmission;
inline constexpr auto ClientPAYLOAD_ENCODING_UNSPECIFIED = static_cast<::harmos::host::v1::PayloadEncoding>(0);
inline constexpr auto ClientPAYLOAD_ENCODING_POSTCARD_V1 = static_cast<::harmos::host::v1::PayloadEncoding>(1);
inline constexpr auto ClientSCHEMA_CARDINALITY_REQUIRED = static_cast<::harmos::host::v1::SchemaCardinality>(0);
inline constexpr auto ClientSCHEMA_CARDINALITY_OPTIONAL = static_cast<::harmos::host::v1::SchemaCardinality>(1);
inline constexpr auto ClientSCHEMA_CARDINALITY_REPEATED = static_cast<::harmos::host::v1::SchemaCardinality>(2);
inline constexpr auto ClientSCHEMA_CARDINALITY_VARIANT = static_cast<::harmos::host::v1::SchemaCardinality>(3);
inline constexpr auto ClientSCHEMA_KIND_BOOL = static_cast<::harmos::host::v1::SchemaKind>(0);
inline constexpr auto ClientSCHEMA_KIND_UINT32 = static_cast<::harmos::host::v1::SchemaKind>(1);
inline constexpr auto ClientSCHEMA_KIND_UINT64 = static_cast<::harmos::host::v1::SchemaKind>(2);
inline constexpr auto ClientSCHEMA_KIND_INT32 = static_cast<::harmos::host::v1::SchemaKind>(3);
inline constexpr auto ClientSCHEMA_KIND_INT64 = static_cast<::harmos::host::v1::SchemaKind>(4);
inline constexpr auto ClientSCHEMA_KIND_FLOAT32 = static_cast<::harmos::host::v1::SchemaKind>(5);
inline constexpr auto ClientSCHEMA_KIND_FLOAT64 = static_cast<::harmos::host::v1::SchemaKind>(6);
inline constexpr auto ClientSCHEMA_KIND_STRING = static_cast<::harmos::host::v1::SchemaKind>(7);
inline constexpr auto ClientSCHEMA_KIND_BYTES = static_cast<::harmos::host::v1::SchemaKind>(8);
inline constexpr auto ClientSCHEMA_KIND_MESSAGE = static_cast<::harmos::host::v1::SchemaKind>(9);
inline constexpr auto ClientSCHEMA_FORM_OBJECT = static_cast<::harmos::host::v1::SchemaForm>(0);
inline constexpr auto ClientSCHEMA_FORM_UNIT = static_cast<::harmos::host::v1::SchemaForm>(1);
inline constexpr auto ClientSCHEMA_FORM_VALUE = static_cast<::harmos::host::v1::SchemaForm>(2);
inline constexpr auto ClientSCHEMA_FORM_TUPLE = static_cast<::harmos::host::v1::SchemaForm>(3);
inline constexpr auto ClientSCHEMA_FORM_EXTERNAL = static_cast<::harmos::host::v1::SchemaForm>(4);
inline constexpr auto ClientSCHEMA_FORM_INTERNAL = static_cast<::harmos::host::v1::SchemaForm>(5);
inline constexpr auto ClientSCHEMA_FORM_ADJACENT = static_cast<::harmos::host::v1::SchemaForm>(6);
inline constexpr auto ClientSCHEMA_FORM_UNTAGGED = static_cast<::harmos::host::v1::SchemaForm>(7);
inline constexpr auto ClientSCHEMA_OMISSION_OMIT_NONE = static_cast<::harmos::host::v1::SchemaOmission>(0);
inline constexpr auto ClientSCHEMA_OMISSION_OMIT_EMPTY = static_cast<::harmos::host::v1::SchemaOmission>(1);
using ClientSchemasListResponse = ::harmos::host::v1::SchemasListResponse;
using ClientSchemaDescription = ::harmos::host::v1::SchemaDescription;
using ClientDiagnosticsGetResponse = ::harmos::host::v1::status::DiagnosticsGetResponse;
using SharedPageRequest = ::harmos::model::v1::PageRequest;
using ClientLogsListResponse = ::harmos::host::v1::status::LogsListResponse;
using ClientLogsWatchResponse = ::harmos::host::v1::status::LogsWatchResponse;
using ClientEvent = ::harmos::host::v1::status::Event;
using ClientTraceEventsListResponse = ::harmos::host::v1::status::TraceEventsListResponse;
using ClientTraceEventsWatchResponse = ::harmos::host::v1::status::TraceEventsWatchResponse;
using ClientStatusGetResponse = ::harmos::host::v1::StatusGetResponse;
using ClientStatusWatchResponse = ::harmos::host::v1::StatusWatchResponse;
using ClientHostIdentity = ::harmos::host::v1::HostIdentity;
using ClientHealthComponent = ::harmos::host::v1::HealthComponent;
using ClientHostStatus = ::harmos::host::v1::HostStatus;
using ClientComponentState = ::harmos::host::v1::ComponentState;
inline constexpr auto ClientCOMPONENT_STATE_UNSPECIFIED = static_cast<::harmos::host::v1::ComponentState>(0);
inline constexpr auto ClientCOMPONENT_STATE_HEALTHY = static_cast<::harmos::host::v1::ComponentState>(1);
inline constexpr auto ClientCOMPONENT_STATE_DEGRADED = static_cast<::harmos::host::v1::ComponentState>(2);
inline constexpr auto ClientCOMPONENT_STATE_UNAVAILABLE = static_cast<::harmos::host::v1::ComponentState>(3);
using ClientStreamsListResponse = ::harmos::host::v1::StreamsListResponse;
using ClientStreamIdentity = ::harmos::host::v1::StreamIdentity;
using SharedTimeGetResponse = ::harmos::host::v1::TimeGetResponse;
using SharedTimeSleepResponse = ::harmos::host::v1::TimeSleepResponse;
using SharedStorageTransferSettings = ::harmos::model::v1::StorageTransferSettings;
using SharedMessageSettings = ::harmos::model::v1::MessageSettings;
using SharedTransferFraming = ::harmos::model::v1::TransferFraming;
inline constexpr auto SharedTRANSFER_FRAMING_UNSPECIFIED = static_cast<::harmos::model::v1::TransferFraming>(0);
inline constexpr auto SharedTRANSFER_FRAMING_BYTES = static_cast<::harmos::model::v1::TransferFraming>(512);
} // namespace harmos::detail
