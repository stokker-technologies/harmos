// @generated from protocol descriptors; do not edit.
#pragma once
#include "harmos/client/v1/access/certificates.pb.h"
#include "harmos/client/v1/access/listener.pb.h"
#include "harmos/client/v1/access/pairings.pb.h"
#include "harmos/client/v1/invocations.pb.h"
#include "harmos/client/v1/operations.pb.h"
#include "harmos/client/v1/plugins.pb.h"
#include "harmos/client/v1/plugins/settings.pb.h"
#include "harmos/client/v1/schemas.pb.h"
#include "harmos/client/v1/status.pb.h"
#include "harmos/client/v1/status/diagnostics.pb.h"
#include "harmos/client/v1/status/logs.pb.h"
#include "harmos/client/v1/status/trace_events.pb.h"
#include "harmos/client/v1/streams.pb.h"
#include "harmos/client/v1/time.pb.h"
#include "harmos/plugin/host/v1/invocations.pb.h"
#include "harmos/plugin/host/v1/operations.pb.h"
#include "harmos/plugin/host/v1/settings.pb.h"
#include "harmos/plugin/host/v1/time.pb.h"
#include "harmos/v1/description.pb.h"
#include "harmos/v1/failure.pb.h"
#include "harmos/v1/file.pb.h"
#include "harmos/v1/invocation.pb.h"
#include "harmos/v1/operation.pb.h"
#include "harmos/v1/page.pb.h"
#include "harmos/v1/policy.pb.h"
#include "harmos/v1/position.pb.h"
#include "harmos/v1/receipt.pb.h"
#include "harmos/v1/schema.pb.h"
#include "harmos/v1/setting.pb.h"
#include "harmos/v1/stream.pb.h"
#include "harmos/v1/transfer.pb.h"
namespace harmos::detail {
inline constexpr char stream_fingerprint_prefix[] = "harmos.protobuf.stream.v1\0";
using ClientCertificatesCreateRequest = ::harmos::client::v1::access::CertificatesCreateRequest;
using ClientListenerGetRequest = ::harmos::client::v1::access::ListenerGetRequest;
using ClientListenerSetRequest = ::harmos::client::v1::access::ListenerSetRequest;
using ClientPairingsCreateRequest = ::harmos::client::v1::access::PairingsCreateRequest;
using ClientPairingsListRequest = ::harmos::client::v1::access::PairingsListRequest;
using ClientPairingsDeleteRequest = ::harmos::client::v1::access::PairingsDeleteRequest;
using SharedRequest = ::harmos::v1::Request;
using SharedResponse = ::harmos::v1::Response;
using SharedChildRequest = ::harmos::v1::ChildRequest;
using SharedChildCancel = ::harmos::v1::ChildCancel;
using SharedChildResponse = ::harmos::v1::ChildResponse;
using ClientInvocationsInvokeRequest = ::harmos::client::v1::InvocationsInvokeRequest;
using ClientInvocationsGetRequest = ::harmos::client::v1::InvocationsGetRequest;
using ClientInvocationsCancelRequest = ::harmos::client::v1::InvocationsCancelRequest;
using ClientInvocationsWatchRequest = ::harmos::client::v1::InvocationsWatchRequest;
using ClientOperationsListRequest = ::harmos::client::v1::OperationsListRequest;
using ClientOperationsGetRequest = ::harmos::client::v1::OperationsGetRequest;
using ClientSettingsListRequest = ::harmos::client::v1::plugins::SettingsListRequest;
using ClientSettingsSetRequest = ::harmos::client::v1::plugins::SettingsSetRequest;
using ClientPluginsGetRequest = ::harmos::client::v1::PluginsGetRequest;
using ClientPluginsListRequest = ::harmos::client::v1::PluginsListRequest;
using ClientPluginsWatchRequest = ::harmos::client::v1::PluginsWatchRequest;
using ClientPluginsCreateRequest = ::harmos::client::v1::PluginsCreateRequest;
using ClientPluginsSetRequest = ::harmos::client::v1::PluginsSetRequest;
using ClientPluginsDeleteRequest = ::harmos::client::v1::PluginsDeleteRequest;
using ClientPluginsRestartRequest = ::harmos::client::v1::PluginsRestartRequest;
using ClientPluginsReplaceRequest = ::harmos::client::v1::PluginsReplaceRequest;
using ClientSchemasListRequest = ::harmos::client::v1::SchemasListRequest;
using ClientDiagnosticsGetRequest = ::harmos::client::v1::status::DiagnosticsGetRequest;
using ClientLogsListRequest = ::harmos::client::v1::status::LogsListRequest;
using ClientLogsWatchRequest = ::harmos::client::v1::status::LogsWatchRequest;
using ClientTraceEventsListRequest = ::harmos::client::v1::status::TraceEventsListRequest;
using ClientTraceEventsWatchRequest = ::harmos::client::v1::status::TraceEventsWatchRequest;
using ClientStatusGetRequest = ::harmos::client::v1::StatusGetRequest;
using ClientStatusWatchRequest = ::harmos::client::v1::StatusWatchRequest;
using ClientStreamsListRequest = ::harmos::client::v1::StreamsListRequest;
using ClientTimeGetRequest = ::harmos::client::v1::TimeGetRequest;
using ClientTimeSleepRequest = ::harmos::client::v1::TimeSleepRequest;
using PluginInvocationsGetRequest = ::harmos::plugin::host::v1::InvocationsGetRequest;
using PluginInvocationsCancelRequest = ::harmos::plugin::host::v1::InvocationsCancelRequest;
using PluginInvocationsWatchRequest = ::harmos::plugin::host::v1::InvocationsWatchRequest;
using PluginOperationsListRequest = ::harmos::plugin::host::v1::OperationsListRequest;
using PluginOperationsGetRequest = ::harmos::plugin::host::v1::OperationsGetRequest;
using PluginSettingsListRequest = ::harmos::plugin::host::v1::SettingsListRequest;
using PluginTimeGetRequest = ::harmos::plugin::host::v1::TimeGetRequest;
using PluginTimeSleepRequest = ::harmos::plugin::host::v1::TimeSleepRequest;
using ClientCertificatesCreateResponse = ::harmos::client::v1::access::CertificatesCreateResponse;
using ClientListenerGetResponse = ::harmos::client::v1::access::ListenerGetResponse;
using ClientListenerSetResponse = ::harmos::client::v1::access::ListenerSetResponse;
using ClientListenerValue = ::harmos::client::v1::access::ListenerValue;
using SharedHistory = ::harmos::v1::History;
using SharedPosition = ::harmos::v1::Position;
using SharedResourceRevision = ::harmos::v1::ResourceRevision;
using SharedReadCut = ::harmos::v1::ReadCut;
using ClientPairingsCreateResponse = ::harmos::client::v1::access::PairingsCreateResponse;
using ClientPairingsListResponse = ::harmos::client::v1::access::PairingsListResponse;
using ClientPairingsDeleteResponse = ::harmos::client::v1::access::PairingsDeleteResponse;
using ClientPairing = ::harmos::client::v1::access::Pairing;
using SharedFailureValue = ::harmos::v1::Failure;
using SharedClassification = ::harmos::v1::Classification;
inline constexpr auto SharedCLASSIFICATION_UNSPECIFIED = static_cast<::harmos::v1::Classification>(0);
inline constexpr auto SharedCLASSIFICATION_PERMANENT = static_cast<::harmos::v1::Classification>(1);
inline constexpr auto SharedCLASSIFICATION_TRANSIENT = static_cast<::harmos::v1::Classification>(2);
inline constexpr auto SharedCLASSIFICATION_CANCELLED = static_cast<::harmos::v1::Classification>(3);
inline constexpr auto SharedCLASSIFICATION_TIMED_OUT = static_cast<::harmos::v1::Classification>(4);
inline constexpr auto SharedCLASSIFICATION_REFUSED = static_cast<::harmos::v1::Classification>(5);
using SharedOperationPolicy = ::harmos::v1::OperationPolicy;
using SharedRetryPolicy = ::harmos::v1::RetryPolicy;
using SharedCancel = ::harmos::v1::Cancel;
using SharedChildDemand = ::harmos::v1::ChildDemand;
using SharedInstanceState = ::harmos::v1::InstanceState;
using ClientInvocationsInvokeResponse = ::harmos::client::v1::InvocationsInvokeResponse;
using ClientInvocationsGetResponse = ::harmos::client::v1::InvocationsGetResponse;
using ClientInvocationsCancelResponse = ::harmos::client::v1::InvocationsCancelResponse;
using ClientInvocationsWatchResponse = ::harmos::client::v1::InvocationsWatchResponse;
using SharedOperationContract = ::harmos::v1::OperationContract;
using SharedRequirement = ::harmos::v1::Requirement;
using SharedAssignedOperation = ::harmos::v1::AssignedOperation;
using SharedOperationAssignments = ::harmos::v1::OperationAssignments;
using SharedProtobuf = ::harmos::v1::Protobuf;
using SharedGrpc = ::harmos::v1::Grpc;
using SharedGrant = ::harmos::v1::Grant;
using SharedDiscoveredContract = ::harmos::v1::DiscoveredContract;
using SharedDiscoveredOperation = ::harmos::v1::DiscoveredOperation;
using SharedVisibility = ::harmos::v1::Visibility;
inline constexpr auto SharedVISIBILITY_UNSPECIFIED = static_cast<::harmos::v1::Visibility>(0);
inline constexpr auto SharedVISIBILITY_PRIVATE = static_cast<::harmos::v1::Visibility>(1);
inline constexpr auto SharedVISIBILITY_PUBLIC = static_cast<::harmos::v1::Visibility>(2);
inline constexpr auto SharedVISIBILITY_PROTECTED = static_cast<::harmos::v1::Visibility>(3);
using ClientOperationsListResponse = ::harmos::client::v1::OperationsListResponse;
using ClientOperationsGetResponse = ::harmos::client::v1::OperationsGetResponse;
using SharedReceipt = ::harmos::v1::Receipt;
using SharedSettingsDeclarations = ::harmos::v1::SettingsDeclarations;
using SharedSettingDeclaration = ::harmos::v1::SettingDeclaration;
using SharedSettingValue = ::harmos::v1::SettingValue;
using SharedSettingValues = ::harmos::v1::SettingValues;
using ClientSettingsListResponse = ::harmos::client::v1::plugins::SettingsListResponse;
using ClientSettingsSetResponse = ::harmos::client::v1::plugins::SettingsSetResponse;
using SharedStreamDeclaration = ::harmos::v1::StreamDeclaration;
using SharedTick = ::harmos::v1::Tick;
using SharedSamplesSelection = ::harmos::v1::SamplesSelection;
using SharedSample = ::harmos::v1::Sample;
using SharedDeclarations = ::harmos::v1::Declarations;
using SharedSection = ::harmos::v1::Section;
using SharedFileMetadata = ::harmos::v1::FileMetadata;
using SharedFileReference = ::harmos::v1::FileReference;
using SharedFile = ::harmos::v1::File;
using SharedUpload = ::harmos::v1::Upload;
using SharedUploadState = ::harmos::v1::UploadState;
inline constexpr auto SharedUPLOAD_STATE_UNSPECIFIED = static_cast<::harmos::v1::UploadState>(0);
inline constexpr auto SharedUPLOAD_STATE_PRODUCING = static_cast<::harmos::v1::UploadState>(1);
inline constexpr auto SharedUPLOAD_STATE_FINISHED = static_cast<::harmos::v1::UploadState>(2);
inline constexpr auto SharedUPLOAD_STATE_ABANDONED = static_cast<::harmos::v1::UploadState>(3);
using ClientPluginsGetResponse = ::harmos::client::v1::PluginsGetResponse;
using ClientPluginsListResponse = ::harmos::client::v1::PluginsListResponse;
using ClientPluginsWatchResponse = ::harmos::client::v1::PluginsWatchResponse;
using ClientPluginsCreateResponse = ::harmos::client::v1::PluginsCreateResponse;
using ClientPluginsSetResponse = ::harmos::client::v1::PluginsSetResponse;
using ClientPluginsDeleteResponse = ::harmos::client::v1::PluginsDeleteResponse;
using ClientPluginsRestartResponse = ::harmos::client::v1::PluginsRestartResponse;
using ClientPluginsReplaceResponse = ::harmos::client::v1::PluginsReplaceResponse;
using ClientPluginValue = ::harmos::client::v1::PluginValue;
using ClientPluginDescription = ::harmos::client::v1::PluginDescription;
using ClientPluginRemote = ::harmos::client::v1::PluginRemote;
using ClientPluginManaged = ::harmos::client::v1::PluginManaged;
using ClientPluginGuest = ::harmos::client::v1::PluginGuest;
using ClientPluginPlacement = ::harmos::client::v1::PluginPlacement;
using ClientPluginProcess = ::harmos::client::v1::PluginProcess;
using ClientPluginEntry = ::harmos::client::v1::PluginEntry;
using ClientPluginSnapshot = ::harmos::client::v1::PluginSnapshot;
using ClientPluginChange = ::harmos::client::v1::PluginChange;
using ClientPluginState = ::harmos::client::v1::PluginState;
inline constexpr auto ClientPLUGIN_STATE_UNSPECIFIED = static_cast<::harmos::client::v1::PluginState>(0);
inline constexpr auto ClientPLUGIN_STATE_CONNECTING = static_cast<::harmos::client::v1::PluginState>(1);
inline constexpr auto ClientPLUGIN_STATE_READY = static_cast<::harmos::client::v1::PluginState>(2);
inline constexpr auto ClientPLUGIN_STATE_UNAVAILABLE = static_cast<::harmos::client::v1::PluginState>(3);
inline constexpr auto ClientPLUGIN_STATE_STOPPED = static_cast<::harmos::client::v1::PluginState>(4);
inline constexpr auto ClientPLUGIN_STATE_STOPPING = static_cast<::harmos::client::v1::PluginState>(5);
using SharedSchemaKey = ::harmos::v1::SchemaKey;
using SharedSchemaField = ::harmos::v1::SchemaField;
using SharedSchemaDefinition = ::harmos::v1::SchemaDefinition;
using SharedSchemaRepresentation = ::harmos::v1::SchemaRepresentation;
using SharedSchemaGraph = ::harmos::v1::SchemaGraph;
using SharedSchemaInput = ::harmos::v1::SchemaInput;
using SharedPayloadEncoding = ::harmos::v1::PayloadEncoding;
using SharedSchemaCardinality = ::harmos::v1::SchemaCardinality;
using SharedSchemaKind = ::harmos::v1::SchemaKind;
using SharedSchemaForm = ::harmos::v1::SchemaForm;
using SharedSchemaOmission = ::harmos::v1::SchemaOmission;
inline constexpr auto SharedPAYLOAD_ENCODING_UNSPECIFIED = static_cast<::harmos::v1::PayloadEncoding>(0);
inline constexpr auto SharedPAYLOAD_ENCODING_POSTCARD_V1 = static_cast<::harmos::v1::PayloadEncoding>(1);
inline constexpr auto SharedSCHEMA_CARDINALITY_REQUIRED = static_cast<::harmos::v1::SchemaCardinality>(0);
inline constexpr auto SharedSCHEMA_CARDINALITY_OPTIONAL = static_cast<::harmos::v1::SchemaCardinality>(1);
inline constexpr auto SharedSCHEMA_CARDINALITY_REPEATED = static_cast<::harmos::v1::SchemaCardinality>(2);
inline constexpr auto SharedSCHEMA_CARDINALITY_VARIANT = static_cast<::harmos::v1::SchemaCardinality>(3);
inline constexpr auto SharedSCHEMA_KIND_BOOL = static_cast<::harmos::v1::SchemaKind>(0);
inline constexpr auto SharedSCHEMA_KIND_UINT32 = static_cast<::harmos::v1::SchemaKind>(1);
inline constexpr auto SharedSCHEMA_KIND_UINT64 = static_cast<::harmos::v1::SchemaKind>(2);
inline constexpr auto SharedSCHEMA_KIND_INT32 = static_cast<::harmos::v1::SchemaKind>(3);
inline constexpr auto SharedSCHEMA_KIND_INT64 = static_cast<::harmos::v1::SchemaKind>(4);
inline constexpr auto SharedSCHEMA_KIND_FLOAT32 = static_cast<::harmos::v1::SchemaKind>(5);
inline constexpr auto SharedSCHEMA_KIND_FLOAT64 = static_cast<::harmos::v1::SchemaKind>(6);
inline constexpr auto SharedSCHEMA_KIND_STRING = static_cast<::harmos::v1::SchemaKind>(7);
inline constexpr auto SharedSCHEMA_KIND_BYTES = static_cast<::harmos::v1::SchemaKind>(8);
inline constexpr auto SharedSCHEMA_KIND_MESSAGE = static_cast<::harmos::v1::SchemaKind>(9);
inline constexpr auto SharedSCHEMA_FORM_OBJECT = static_cast<::harmos::v1::SchemaForm>(0);
inline constexpr auto SharedSCHEMA_FORM_UNIT = static_cast<::harmos::v1::SchemaForm>(1);
inline constexpr auto SharedSCHEMA_FORM_VALUE = static_cast<::harmos::v1::SchemaForm>(2);
inline constexpr auto SharedSCHEMA_FORM_TUPLE = static_cast<::harmos::v1::SchemaForm>(3);
inline constexpr auto SharedSCHEMA_FORM_EXTERNAL = static_cast<::harmos::v1::SchemaForm>(4);
inline constexpr auto SharedSCHEMA_FORM_INTERNAL = static_cast<::harmos::v1::SchemaForm>(5);
inline constexpr auto SharedSCHEMA_FORM_ADJACENT = static_cast<::harmos::v1::SchemaForm>(6);
inline constexpr auto SharedSCHEMA_FORM_UNTAGGED = static_cast<::harmos::v1::SchemaForm>(7);
inline constexpr auto SharedSCHEMA_OMISSION_OMIT_NONE = static_cast<::harmos::v1::SchemaOmission>(0);
inline constexpr auto SharedSCHEMA_OMISSION_OMIT_EMPTY = static_cast<::harmos::v1::SchemaOmission>(1);
using ClientSchemasListResponse = ::harmos::client::v1::SchemasListResponse;
using ClientSchemaDescription = ::harmos::client::v1::SchemaDescription;
using ClientDiagnosticsGetResponse = ::harmos::client::v1::status::DiagnosticsGetResponse;
using SharedPageRequest = ::harmos::v1::PageRequest;
using ClientLogsListResponse = ::harmos::client::v1::status::LogsListResponse;
using ClientLogsWatchResponse = ::harmos::client::v1::status::LogsWatchResponse;
using ClientEvent = ::harmos::client::v1::status::Event;
using ClientTraceEventsListResponse = ::harmos::client::v1::status::TraceEventsListResponse;
using ClientTraceEventsWatchResponse = ::harmos::client::v1::status::TraceEventsWatchResponse;
using ClientStatusGetResponse = ::harmos::client::v1::StatusGetResponse;
using ClientStatusWatchResponse = ::harmos::client::v1::StatusWatchResponse;
using ClientHostIdentity = ::harmos::client::v1::HostIdentity;
using ClientHealthComponent = ::harmos::client::v1::HealthComponent;
using ClientHostStatus = ::harmos::client::v1::HostStatus;
using ClientComponentState = ::harmos::client::v1::ComponentState;
inline constexpr auto ClientCOMPONENT_STATE_UNSPECIFIED = static_cast<::harmos::client::v1::ComponentState>(0);
inline constexpr auto ClientCOMPONENT_STATE_HEALTHY = static_cast<::harmos::client::v1::ComponentState>(1);
inline constexpr auto ClientCOMPONENT_STATE_DEGRADED = static_cast<::harmos::client::v1::ComponentState>(2);
inline constexpr auto ClientCOMPONENT_STATE_UNAVAILABLE = static_cast<::harmos::client::v1::ComponentState>(3);
using ClientStreamsListResponse = ::harmos::client::v1::StreamsListResponse;
using ClientStreamIdentity = ::harmos::client::v1::StreamIdentity;
using ClientTimeGetResponse = ::harmos::client::v1::TimeGetResponse;
using ClientTimeSleepResponse = ::harmos::client::v1::TimeSleepResponse;
using PluginInvocationsGetResponse = ::harmos::plugin::host::v1::InvocationsGetResponse;
using PluginInvocationsCancelResponse = ::harmos::plugin::host::v1::InvocationsCancelResponse;
using PluginInvocationsWatchResponse = ::harmos::plugin::host::v1::InvocationsWatchResponse;
using PluginOperationsListResponse = ::harmos::plugin::host::v1::OperationsListResponse;
using PluginOperationsGetResponse = ::harmos::plugin::host::v1::OperationsGetResponse;
using PluginSettingsListResponse = ::harmos::plugin::host::v1::SettingsListResponse;
using PluginTimeGetResponse = ::harmos::plugin::host::v1::TimeGetResponse;
using PluginTimeSleepResponse = ::harmos::plugin::host::v1::TimeSleepResponse;
using SharedStorageTransferSettings = ::harmos::v1::StorageTransferSettings;
using SharedMessageSettings = ::harmos::v1::MessageSettings;
using SharedTransferFraming = ::harmos::v1::TransferFraming;
inline constexpr auto SharedTRANSFER_FRAMING_UNSPECIFIED = static_cast<::harmos::v1::TransferFraming>(0);
inline constexpr auto SharedTRANSFER_FRAMING_BYTES = static_cast<::harmos::v1::TransferFraming>(512);
} // namespace harmos::detail
