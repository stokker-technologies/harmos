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
namespace harmos::detail::packages {
namespace harmos::host::access {
using ::harmos::host::v1::access::CertificatesCreateRequest;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::CertificatesCreateResponse;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::Certificates;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::ListenerGetRequest;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::ListenerGetResponse;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::Listener;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::ListenerSetRequest;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::ListenerSetResponse;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::Listener;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::PairingsCreateRequest;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::PairingsCreateResponse;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::Pairings;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::PairingsListRequest;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::PairingsListResponse;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::Pairings;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::PairingsDeleteRequest;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::PairingsDeleteResponse;
}
namespace harmos::host::access {
using ::harmos::host::v1::access::Pairings;
}
namespace harmos::host {
using ::harmos::host::v1::InvocationsCreateRequest;
}
namespace harmos::host {
using ::harmos::host::v1::InvocationsCreateResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Invocations;
}
namespace harmos::host {
using ::harmos::host::v1::InvocationsGetRequest;
}
namespace harmos::host {
using ::harmos::host::v1::InvocationsGetResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Invocations;
}
namespace harmos::host {
using ::harmos::host::v1::InvocationsCancelRequest;
}
namespace harmos::host {
using ::harmos::host::v1::InvocationsCancelResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Invocations;
}
namespace harmos::host {
using ::harmos::host::v1::InvocationsWatchRequest;
}
namespace harmos::host {
using ::harmos::host::v1::InvocationsWatchResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Invocations;
}
namespace harmos::host {
using ::harmos::host::v1::OperationsListRequest;
}
namespace harmos::host {
using ::harmos::host::v1::OperationsListResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Operations;
}
namespace harmos::host {
using ::harmos::host::v1::OperationsGetRequest;
}
namespace harmos::host {
using ::harmos::host::v1::OperationsGetResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Operations;
}
namespace harmos::host::plugins {
using ::harmos::host::v1::plugins::SettingsListRequest;
}
namespace harmos::host::plugins {
using ::harmos::host::v1::plugins::SettingsListResponse;
}
namespace harmos::host::plugins {
using ::harmos::host::v1::plugins::Settings;
}
namespace harmos::host::plugins {
using ::harmos::host::v1::plugins::SettingsSetRequest;
}
namespace harmos::host::plugins {
using ::harmos::host::v1::plugins::SettingsSetResponse;
}
namespace harmos::host::plugins {
using ::harmos::host::v1::plugins::Settings;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsGetRequest;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsGetResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Plugins;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsListRequest;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsListResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Plugins;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsWatchRequest;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsWatchResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Plugins;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsCreateRequest;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsCreateResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Plugins;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsSetRequest;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsSetResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Plugins;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsDeleteRequest;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsDeleteResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Plugins;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsRestartRequest;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsRestartResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Plugins;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsReplaceRequest;
}
namespace harmos::host {
using ::harmos::host::v1::PluginsReplaceResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Plugins;
}
namespace harmos::host {
using ::harmos::host::v1::SchemasListRequest;
}
namespace harmos::host {
using ::harmos::host::v1::SchemasListResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Schemas;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::DiagnosticsGetRequest;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::DiagnosticsGetResponse;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::Diagnostics;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::LogsListRequest;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::LogsListResponse;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::Logs;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::LogsWatchRequest;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::LogsWatchResponse;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::Logs;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::TraceEventsListRequest;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::TraceEventsListResponse;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::TraceEvents;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::TraceEventsWatchRequest;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::TraceEventsWatchResponse;
}
namespace harmos::host::status {
using ::harmos::host::v1::status::TraceEvents;
}
namespace harmos::host {
using ::harmos::host::v1::StatusGetRequest;
}
namespace harmos::host {
using ::harmos::host::v1::StatusGetResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Status;
}
namespace harmos::host {
using ::harmos::host::v1::StatusWatchRequest;
}
namespace harmos::host {
using ::harmos::host::v1::StatusWatchResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Status;
}
namespace harmos::host {
using ::harmos::host::v1::StreamsListRequest;
}
namespace harmos::host {
using ::harmos::host::v1::StreamsListResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Streams;
}
namespace harmos::host {
using ::harmos::host::v1::TimeGetRequest;
}
namespace harmos::host {
using ::harmos::host::v1::TimeGetResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Time;
}
namespace harmos::host {
using ::harmos::host::v1::TimeSleepRequest;
}
namespace harmos::host {
using ::harmos::host::v1::TimeSleepResponse;
}
namespace harmos::host {
using ::harmos::host::v1::Time;
}
} // namespace harmos::detail::packages
