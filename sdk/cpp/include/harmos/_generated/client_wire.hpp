// @generated from protocol descriptors; do not edit.
#pragma once
#include "harmos/host/v1/access/certificates.grpc.pb.h"
#include "harmos/host/v1/access/listener.grpc.pb.h"
#include "harmos/host/v1/access/pairings.grpc.pb.h"
#include "harmos/host/v1/invocations.grpc.pb.h"
#include "harmos/host/v1/operations.grpc.pb.h"
#include "harmos/host/v1/plugins.grpc.pb.h"
#include "harmos/host/v1/plugins/settings.grpc.pb.h"
#include "harmos/host/v1/schemas.grpc.pb.h"
#include "harmos/host/v1/status.grpc.pb.h"
#include "harmos/host/v1/status/diagnostics.grpc.pb.h"
#include "harmos/host/v1/status/logs.grpc.pb.h"
#include "harmos/host/v1/status/trace_events.grpc.pb.h"
#include "harmos/host/v1/streams.grpc.pb.h"
#include "harmos/host/v1/time.grpc.pb.h"
namespace harmos::detail {
using AccessCertificatesCreateRequest = ::harmos::host::v1::access::CertificatesCreateRequest;
using AccessCertificatesCreateResponse = ::harmos::host::v1::access::CertificatesCreateResponse;
using AccessCertificatesCreateService = ::harmos::host::v1::access::Certificates;
using AccessListenerGetRequest = ::harmos::host::v1::access::ListenerGetRequest;
using AccessListenerGetResponse = ::harmos::host::v1::access::ListenerGetResponse;
using AccessListenerGetService = ::harmos::host::v1::access::Listener;
using AccessListenerSetRequest = ::harmos::host::v1::access::ListenerSetRequest;
using AccessListenerSetResponse = ::harmos::host::v1::access::ListenerSetResponse;
using AccessListenerSetService = ::harmos::host::v1::access::Listener;
using AccessPairingsCreateRequest = ::harmos::host::v1::access::PairingsCreateRequest;
using AccessPairingsCreateResponse = ::harmos::host::v1::access::PairingsCreateResponse;
using AccessPairingsCreateService = ::harmos::host::v1::access::Pairings;
using AccessPairingsListRequest = ::harmos::host::v1::access::PairingsListRequest;
using AccessPairingsListResponse = ::harmos::host::v1::access::PairingsListResponse;
using AccessPairingsListService = ::harmos::host::v1::access::Pairings;
using AccessPairingsDeleteRequest = ::harmos::host::v1::access::PairingsDeleteRequest;
using AccessPairingsDeleteResponse = ::harmos::host::v1::access::PairingsDeleteResponse;
using AccessPairingsDeleteService = ::harmos::host::v1::access::Pairings;
using InvocationsCreateRequest = ::harmos::host::v1::InvocationsCreateRequest;
using InvocationsCreateResponse = ::harmos::host::v1::InvocationsCreateResponse;
using InvocationsCreateService = ::harmos::host::v1::Invocations;
using InvocationsGetRequest = ::harmos::host::v1::InvocationsGetRequest;
using InvocationsGetResponse = ::harmos::host::v1::InvocationsGetResponse;
using InvocationsGetService = ::harmos::host::v1::Invocations;
using InvocationsCancelRequest = ::harmos::host::v1::InvocationsCancelRequest;
using InvocationsCancelResponse = ::harmos::host::v1::InvocationsCancelResponse;
using InvocationsCancelService = ::harmos::host::v1::Invocations;
using InvocationsWatchRequest = ::harmos::host::v1::InvocationsWatchRequest;
using InvocationsWatchResponse = ::harmos::host::v1::InvocationsWatchResponse;
using InvocationsWatchService = ::harmos::host::v1::Invocations;
using OperationsListRequest = ::harmos::host::v1::OperationsListRequest;
using OperationsListResponse = ::harmos::host::v1::OperationsListResponse;
using OperationsListService = ::harmos::host::v1::Operations;
using OperationsGetRequest = ::harmos::host::v1::OperationsGetRequest;
using OperationsGetResponse = ::harmos::host::v1::OperationsGetResponse;
using OperationsGetService = ::harmos::host::v1::Operations;
using PluginsSettingsListRequest = ::harmos::host::v1::plugins::SettingsListRequest;
using PluginsSettingsListResponse = ::harmos::host::v1::plugins::SettingsListResponse;
using PluginsSettingsListService = ::harmos::host::v1::plugins::Settings;
using PluginsSettingsSetRequest = ::harmos::host::v1::plugins::SettingsSetRequest;
using PluginsSettingsSetResponse = ::harmos::host::v1::plugins::SettingsSetResponse;
using PluginsSettingsSetService = ::harmos::host::v1::plugins::Settings;
using PluginsGetRequest = ::harmos::host::v1::PluginsGetRequest;
using PluginsGetResponse = ::harmos::host::v1::PluginsGetResponse;
using PluginsGetService = ::harmos::host::v1::Plugins;
using PluginsListRequest = ::harmos::host::v1::PluginsListRequest;
using PluginsListResponse = ::harmos::host::v1::PluginsListResponse;
using PluginsListService = ::harmos::host::v1::Plugins;
using PluginsWatchRequest = ::harmos::host::v1::PluginsWatchRequest;
using PluginsWatchResponse = ::harmos::host::v1::PluginsWatchResponse;
using PluginsWatchService = ::harmos::host::v1::Plugins;
using PluginsCreateRequest = ::harmos::host::v1::PluginsCreateRequest;
using PluginsCreateResponse = ::harmos::host::v1::PluginsCreateResponse;
using PluginsCreateService = ::harmos::host::v1::Plugins;
using PluginsSetRequest = ::harmos::host::v1::PluginsSetRequest;
using PluginsSetResponse = ::harmos::host::v1::PluginsSetResponse;
using PluginsSetService = ::harmos::host::v1::Plugins;
using PluginsDeleteRequest = ::harmos::host::v1::PluginsDeleteRequest;
using PluginsDeleteResponse = ::harmos::host::v1::PluginsDeleteResponse;
using PluginsDeleteService = ::harmos::host::v1::Plugins;
using PluginsRestartRequest = ::harmos::host::v1::PluginsRestartRequest;
using PluginsRestartResponse = ::harmos::host::v1::PluginsRestartResponse;
using PluginsRestartService = ::harmos::host::v1::Plugins;
using PluginsReplaceRequest = ::harmos::host::v1::PluginsReplaceRequest;
using PluginsReplaceResponse = ::harmos::host::v1::PluginsReplaceResponse;
using PluginsReplaceService = ::harmos::host::v1::Plugins;
using SchemasListRequest = ::harmos::host::v1::SchemasListRequest;
using SchemasListResponse = ::harmos::host::v1::SchemasListResponse;
using SchemasListService = ::harmos::host::v1::Schemas;
using StatusDiagnosticsGetRequest = ::harmos::host::v1::status::DiagnosticsGetRequest;
using StatusDiagnosticsGetResponse = ::harmos::host::v1::status::DiagnosticsGetResponse;
using StatusDiagnosticsGetService = ::harmos::host::v1::status::Diagnostics;
using StatusLogsListRequest = ::harmos::host::v1::status::LogsListRequest;
using StatusLogsListResponse = ::harmos::host::v1::status::LogsListResponse;
using StatusLogsListService = ::harmos::host::v1::status::Logs;
using StatusLogsWatchRequest = ::harmos::host::v1::status::LogsWatchRequest;
using StatusLogsWatchResponse = ::harmos::host::v1::status::LogsWatchResponse;
using StatusLogsWatchService = ::harmos::host::v1::status::Logs;
using StatusTraceEventsListRequest = ::harmos::host::v1::status::TraceEventsListRequest;
using StatusTraceEventsListResponse = ::harmos::host::v1::status::TraceEventsListResponse;
using StatusTraceEventsListService = ::harmos::host::v1::status::TraceEvents;
using StatusTraceEventsWatchRequest = ::harmos::host::v1::status::TraceEventsWatchRequest;
using StatusTraceEventsWatchResponse = ::harmos::host::v1::status::TraceEventsWatchResponse;
using StatusTraceEventsWatchService = ::harmos::host::v1::status::TraceEvents;
using StatusGetRequest = ::harmos::host::v1::StatusGetRequest;
using StatusGetResponse = ::harmos::host::v1::StatusGetResponse;
using StatusGetService = ::harmos::host::v1::Status;
using StatusWatchRequest = ::harmos::host::v1::StatusWatchRequest;
using StatusWatchResponse = ::harmos::host::v1::StatusWatchResponse;
using StatusWatchService = ::harmos::host::v1::Status;
using StreamsListRequest = ::harmos::host::v1::StreamsListRequest;
using StreamsListResponse = ::harmos::host::v1::StreamsListResponse;
using StreamsListService = ::harmos::host::v1::Streams;
using TimeGetRequest = ::harmos::host::v1::TimeGetRequest;
using TimeGetResponse = ::harmos::host::v1::TimeGetResponse;
using TimeGetService = ::harmos::host::v1::Time;
using TimeSleepRequest = ::harmos::host::v1::TimeSleepRequest;
using TimeSleepResponse = ::harmos::host::v1::TimeSleepResponse;
using TimeSleepService = ::harmos::host::v1::Time;
} // namespace harmos::detail
