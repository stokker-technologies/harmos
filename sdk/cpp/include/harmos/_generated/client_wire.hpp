// @generated from protocol descriptors; do not edit.
#pragma once
#include "harmos/client/v1/access/certificates.grpc.pb.h"
#include "harmos/client/v1/access/listener.grpc.pb.h"
#include "harmos/client/v1/access/pairings.grpc.pb.h"
#include "harmos/client/v1/invocations.grpc.pb.h"
#include "harmos/client/v1/operations.grpc.pb.h"
#include "harmos/client/v1/plugins.grpc.pb.h"
#include "harmos/client/v1/plugins/settings.grpc.pb.h"
#include "harmos/client/v1/schemas.grpc.pb.h"
#include "harmos/client/v1/status.grpc.pb.h"
#include "harmos/client/v1/status/diagnostics.grpc.pb.h"
#include "harmos/client/v1/status/logs.grpc.pb.h"
#include "harmos/client/v1/status/trace_events.grpc.pb.h"
#include "harmos/client/v1/streams.grpc.pb.h"
#include "harmos/client/v1/time.grpc.pb.h"
namespace harmos::detail {
using AccessCertificatesCreateRequest = ::harmos::client::v1::access::CertificatesCreateRequest;
using AccessCertificatesCreateResponse = ::harmos::client::v1::access::CertificatesCreateResponse;
using AccessCertificatesCreateService = ::harmos::client::v1::access::Certificates;
using AccessListenerGetRequest = ::harmos::client::v1::access::ListenerGetRequest;
using AccessListenerGetResponse = ::harmos::client::v1::access::ListenerGetResponse;
using AccessListenerGetService = ::harmos::client::v1::access::Listener;
using AccessListenerSetRequest = ::harmos::client::v1::access::ListenerSetRequest;
using AccessListenerSetResponse = ::harmos::client::v1::access::ListenerSetResponse;
using AccessListenerSetService = ::harmos::client::v1::access::Listener;
using AccessPairingsCreateRequest = ::harmos::client::v1::access::PairingsCreateRequest;
using AccessPairingsCreateResponse = ::harmos::client::v1::access::PairingsCreateResponse;
using AccessPairingsCreateService = ::harmos::client::v1::access::Pairings;
using AccessPairingsListRequest = ::harmos::client::v1::access::PairingsListRequest;
using AccessPairingsListResponse = ::harmos::client::v1::access::PairingsListResponse;
using AccessPairingsListService = ::harmos::client::v1::access::Pairings;
using AccessPairingsDeleteRequest = ::harmos::client::v1::access::PairingsDeleteRequest;
using AccessPairingsDeleteResponse = ::harmos::client::v1::access::PairingsDeleteResponse;
using AccessPairingsDeleteService = ::harmos::client::v1::access::Pairings;
using InvocationsInvokeRequest = ::harmos::client::v1::InvocationsInvokeRequest;
using InvocationsInvokeResponse = ::harmos::client::v1::InvocationsInvokeResponse;
using InvocationsInvokeService = ::harmos::client::v1::Invocations;
using InvocationsGetRequest = ::harmos::client::v1::InvocationsGetRequest;
using InvocationsGetResponse = ::harmos::client::v1::InvocationsGetResponse;
using InvocationsGetService = ::harmos::client::v1::Invocations;
using InvocationsCancelRequest = ::harmos::client::v1::InvocationsCancelRequest;
using InvocationsCancelResponse = ::harmos::client::v1::InvocationsCancelResponse;
using InvocationsCancelService = ::harmos::client::v1::Invocations;
using InvocationsWatchRequest = ::harmos::client::v1::InvocationsWatchRequest;
using InvocationsWatchResponse = ::harmos::client::v1::InvocationsWatchResponse;
using InvocationsWatchService = ::harmos::client::v1::Invocations;
using OperationsListRequest = ::harmos::client::v1::OperationsListRequest;
using OperationsListResponse = ::harmos::client::v1::OperationsListResponse;
using OperationsListService = ::harmos::client::v1::Operations;
using OperationsGetRequest = ::harmos::client::v1::OperationsGetRequest;
using OperationsGetResponse = ::harmos::client::v1::OperationsGetResponse;
using OperationsGetService = ::harmos::client::v1::Operations;
using PluginsSettingsListRequest = ::harmos::client::v1::plugins::SettingsListRequest;
using PluginsSettingsListResponse = ::harmos::client::v1::plugins::SettingsListResponse;
using PluginsSettingsListService = ::harmos::client::v1::plugins::Settings;
using PluginsSettingsSetRequest = ::harmos::client::v1::plugins::SettingsSetRequest;
using PluginsSettingsSetResponse = ::harmos::client::v1::plugins::SettingsSetResponse;
using PluginsSettingsSetService = ::harmos::client::v1::plugins::Settings;
using PluginsGetRequest = ::harmos::client::v1::PluginsGetRequest;
using PluginsGetResponse = ::harmos::client::v1::PluginsGetResponse;
using PluginsGetService = ::harmos::client::v1::Plugins;
using PluginsListRequest = ::harmos::client::v1::PluginsListRequest;
using PluginsListResponse = ::harmos::client::v1::PluginsListResponse;
using PluginsListService = ::harmos::client::v1::Plugins;
using PluginsWatchRequest = ::harmos::client::v1::PluginsWatchRequest;
using PluginsWatchResponse = ::harmos::client::v1::PluginsWatchResponse;
using PluginsWatchService = ::harmos::client::v1::Plugins;
using PluginsCreateRequest = ::harmos::client::v1::PluginsCreateRequest;
using PluginsCreateResponse = ::harmos::client::v1::PluginsCreateResponse;
using PluginsCreateService = ::harmos::client::v1::Plugins;
using PluginsSetRequest = ::harmos::client::v1::PluginsSetRequest;
using PluginsSetResponse = ::harmos::client::v1::PluginsSetResponse;
using PluginsSetService = ::harmos::client::v1::Plugins;
using PluginsDeleteRequest = ::harmos::client::v1::PluginsDeleteRequest;
using PluginsDeleteResponse = ::harmos::client::v1::PluginsDeleteResponse;
using PluginsDeleteService = ::harmos::client::v1::Plugins;
using PluginsRestartRequest = ::harmos::client::v1::PluginsRestartRequest;
using PluginsRestartResponse = ::harmos::client::v1::PluginsRestartResponse;
using PluginsRestartService = ::harmos::client::v1::Plugins;
using PluginsReplaceRequest = ::harmos::client::v1::PluginsReplaceRequest;
using PluginsReplaceResponse = ::harmos::client::v1::PluginsReplaceResponse;
using PluginsReplaceService = ::harmos::client::v1::Plugins;
using SchemasListRequest = ::harmos::client::v1::SchemasListRequest;
using SchemasListResponse = ::harmos::client::v1::SchemasListResponse;
using SchemasListService = ::harmos::client::v1::Schemas;
using StatusDiagnosticsGetRequest = ::harmos::client::v1::status::DiagnosticsGetRequest;
using StatusDiagnosticsGetResponse = ::harmos::client::v1::status::DiagnosticsGetResponse;
using StatusDiagnosticsGetService = ::harmos::client::v1::status::Diagnostics;
using StatusLogsListRequest = ::harmos::client::v1::status::LogsListRequest;
using StatusLogsListResponse = ::harmos::client::v1::status::LogsListResponse;
using StatusLogsListService = ::harmos::client::v1::status::Logs;
using StatusLogsWatchRequest = ::harmos::client::v1::status::LogsWatchRequest;
using StatusLogsWatchResponse = ::harmos::client::v1::status::LogsWatchResponse;
using StatusLogsWatchService = ::harmos::client::v1::status::Logs;
using StatusTraceEventsListRequest = ::harmos::client::v1::status::TraceEventsListRequest;
using StatusTraceEventsListResponse = ::harmos::client::v1::status::TraceEventsListResponse;
using StatusTraceEventsListService = ::harmos::client::v1::status::TraceEvents;
using StatusTraceEventsWatchRequest = ::harmos::client::v1::status::TraceEventsWatchRequest;
using StatusTraceEventsWatchResponse = ::harmos::client::v1::status::TraceEventsWatchResponse;
using StatusTraceEventsWatchService = ::harmos::client::v1::status::TraceEvents;
using StatusGetRequest = ::harmos::client::v1::StatusGetRequest;
using StatusGetResponse = ::harmos::client::v1::StatusGetResponse;
using StatusGetService = ::harmos::client::v1::Status;
using StatusWatchRequest = ::harmos::client::v1::StatusWatchRequest;
using StatusWatchResponse = ::harmos::client::v1::StatusWatchResponse;
using StatusWatchService = ::harmos::client::v1::Status;
using StreamsListRequest = ::harmos::client::v1::StreamsListRequest;
using StreamsListResponse = ::harmos::client::v1::StreamsListResponse;
using StreamsListService = ::harmos::client::v1::Streams;
using TimeGetRequest = ::harmos::client::v1::TimeGetRequest;
using TimeGetResponse = ::harmos::client::v1::TimeGetResponse;
using TimeGetService = ::harmos::client::v1::Time;
using TimeSleepRequest = ::harmos::client::v1::TimeSleepRequest;
using TimeSleepResponse = ::harmos::client::v1::TimeSleepResponse;
using TimeSleepService = ::harmos::client::v1::Time;
} // namespace harmos::detail
