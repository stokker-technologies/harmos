// @generated from the protocol descriptor table; do not edit.
#pragma once
#include <grpcpp/grpcpp.h>
#include <memory>
#include <stdexcept>
#include "harmos/transfer.hpp"
#include <string>
#pragma push_macro("Cancel")
#undef Cancel
#pragma push_macro("Create")
#undef Create
#pragma push_macro("Delete")
#undef Delete
#pragma push_macro("Get")
#undef Get
#pragma push_macro("Invoke")
#undef Invoke
#pragma push_macro("List")
#undef List
#pragma push_macro("Replace")
#undef Replace
#pragma push_macro("Restart")
#undef Restart
#pragma push_macro("Set")
#undef Set
#pragma push_macro("Sleep")
#undef Sleep
#pragma push_macro("Watch")
#undef Watch
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
namespace harmos {
struct AccessCertificatesResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit AccessCertificatesResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::access::CertificatesCreateResponse create(grpc::ClientContext& context, const ::harmos::client::v1::access::CertificatesCreateRequest& request) {
    ::harmos::client::v1::access::CertificatesCreateResponse response;
    const auto status = ::harmos::client::v1::access::Certificates::NewStub(channel)->Create(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Create: ") + status.error_message());
    return response;
  }
};
struct AccessListenerResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit AccessListenerResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::access::ListenerGetResponse get(grpc::ClientContext& context, const ::harmos::client::v1::access::ListenerGetRequest& request) {
    ::harmos::client::v1::access::ListenerGetResponse response;
    const auto status = ::harmos::client::v1::access::Listener::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::access::ListenerSetResponse set(grpc::ClientContext& context, const ::harmos::client::v1::access::ListenerSetRequest& request) {
    ::harmos::client::v1::access::ListenerSetResponse response;
    const auto status = ::harmos::client::v1::access::Listener::NewStub(channel)->Set(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Set: ") + status.error_message());
    return response;
  }
};
struct AccessPairingsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit AccessPairingsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::access::PairingsCreateResponse create(grpc::ClientContext& context, const ::harmos::client::v1::access::PairingsCreateRequest& request) {
    ::harmos::client::v1::access::PairingsCreateResponse response;
    const auto status = ::harmos::client::v1::access::Pairings::NewStub(channel)->Create(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Create: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::access::PairingsListResponse list(grpc::ClientContext& context, const ::harmos::client::v1::access::PairingsListRequest& request) {
    ::harmos::client::v1::access::PairingsListResponse response;
    const auto status = ::harmos::client::v1::access::Pairings::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::access::PairingsDeleteResponse delete_(grpc::ClientContext& context, const ::harmos::client::v1::access::PairingsDeleteRequest& request) {
    ::harmos::client::v1::access::PairingsDeleteResponse response;
    const auto status = ::harmos::client::v1::access::Pairings::NewStub(channel)->Delete(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Delete: ") + status.error_message());
    return response;
  }
};
struct PluginsSettingsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit PluginsSettingsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::plugins::SettingsListResponse list(grpc::ClientContext& context, const ::harmos::client::v1::plugins::SettingsListRequest& request) {
    ::harmos::client::v1::plugins::SettingsListResponse response;
    const auto status = ::harmos::client::v1::plugins::Settings::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::plugins::SettingsSetResponse set(grpc::ClientContext& context, const ::harmos::client::v1::plugins::SettingsSetRequest& request) {
    ::harmos::client::v1::plugins::SettingsSetResponse response;
    const auto status = ::harmos::client::v1::plugins::Settings::NewStub(channel)->Set(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Set: ") + status.error_message());
    return response;
  }
};
struct StatusDiagnosticsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StatusDiagnosticsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::status::DiagnosticsGetResponse get(grpc::ClientContext& context, const ::harmos::client::v1::status::DiagnosticsGetRequest& request) {
    ::harmos::client::v1::status::DiagnosticsGetResponse response;
    const auto status = ::harmos::client::v1::status::Diagnostics::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
};
struct StatusLogsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StatusLogsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::status::LogsListResponse list(grpc::ClientContext& context, const ::harmos::client::v1::status::LogsListRequest& request) {
    ::harmos::client::v1::status::LogsListResponse response;
    const auto status = ::harmos::client::v1::status::Logs::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
  std::unique_ptr<grpc::ClientReader<::harmos::client::v1::status::LogsWatchResponse>> watch(grpc::ClientContext& context, const ::harmos::client::v1::status::LogsWatchRequest& request) {
    return ::harmos::client::v1::status::Logs::NewStub(channel)->Watch(&context, request);
  }
};
struct StatusTraceEventsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StatusTraceEventsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::status::TraceEventsListResponse list(grpc::ClientContext& context, const ::harmos::client::v1::status::TraceEventsListRequest& request) {
    ::harmos::client::v1::status::TraceEventsListResponse response;
    const auto status = ::harmos::client::v1::status::TraceEvents::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
  std::unique_ptr<grpc::ClientReader<::harmos::client::v1::status::TraceEventsWatchResponse>> watch(grpc::ClientContext& context, const ::harmos::client::v1::status::TraceEventsWatchRequest& request) {
    return ::harmos::client::v1::status::TraceEvents::NewStub(channel)->Watch(&context, request);
  }
};
struct AccessResource {
  AccessCertificatesResource certificates;
  AccessListenerResource listener;
  AccessPairingsResource pairings;
  explicit AccessResource(std::shared_ptr<grpc::Channel> channel) : certificates(channel), listener(channel), pairings(channel) {}
};
struct InvocationsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit InvocationsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::InvocationsInvokeResponse invoke(grpc::ClientContext& context, const ::harmos::client::v1::InvocationsInvokeRequest& request) {
    ::harmos::client::v1::InvocationsInvokeResponse response;
    const auto status = ::harmos::client::v1::Invocations::NewStub(channel)->Invoke(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Invoke: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::InvocationsGetResponse get(grpc::ClientContext& context, const ::harmos::client::v1::InvocationsGetRequest& request) {
    ::harmos::client::v1::InvocationsGetResponse response;
    const auto status = ::harmos::client::v1::Invocations::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::InvocationsCancelResponse cancel(grpc::ClientContext& context, const ::harmos::client::v1::InvocationsCancelRequest& request) {
    ::harmos::client::v1::InvocationsCancelResponse response;
    const auto status = ::harmos::client::v1::Invocations::NewStub(channel)->Cancel(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Cancel: ") + status.error_message());
    return response;
  }
  std::unique_ptr<grpc::ClientReader<::harmos::client::v1::InvocationsWatchResponse>> watch(grpc::ClientContext& context, const ::harmos::client::v1::InvocationsWatchRequest& request) {
    return ::harmos::client::v1::Invocations::NewStub(channel)->Watch(&context, request);
  }
};
struct OperationsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit OperationsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::OperationsListResponse list(grpc::ClientContext& context, const ::harmos::client::v1::OperationsListRequest& request) {
    ::harmos::client::v1::OperationsListResponse response;
    const auto status = ::harmos::client::v1::Operations::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::OperationsGetResponse get(grpc::ClientContext& context, const ::harmos::client::v1::OperationsGetRequest& request) {
    ::harmos::client::v1::OperationsGetResponse response;
    const auto status = ::harmos::client::v1::Operations::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
};
struct PluginsResource {
  std::shared_ptr<grpc::Channel> channel;
  PluginsSettingsResource settings;
  explicit PluginsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel), settings(channel) {}
  ::harmos::client::v1::PluginsGetResponse get(grpc::ClientContext& context, const ::harmos::client::v1::PluginsGetRequest& request) {
    ::harmos::client::v1::PluginsGetResponse response;
    const auto status = ::harmos::client::v1::Plugins::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::PluginsListResponse list(grpc::ClientContext& context, const ::harmos::client::v1::PluginsListRequest& request) {
    ::harmos::client::v1::PluginsListResponse response;
    const auto status = ::harmos::client::v1::Plugins::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
  std::unique_ptr<grpc::ClientReader<::harmos::client::v1::PluginsWatchResponse>> watch(grpc::ClientContext& context, const ::harmos::client::v1::PluginsWatchRequest& request) {
    return ::harmos::client::v1::Plugins::NewStub(channel)->Watch(&context, request);
  }
  ::harmos::client::v1::PluginsCreateResponse create(grpc::ClientContext& context, const ::harmos::client::v1::PluginsCreateRequest& request) {
    ::harmos::client::v1::PluginsCreateResponse response;
    const auto status = ::harmos::client::v1::Plugins::NewStub(channel)->Create(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Create: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::PluginsSetResponse set(grpc::ClientContext& context, const ::harmos::client::v1::PluginsSetRequest& request) {
    ::harmos::client::v1::PluginsSetResponse response;
    const auto status = ::harmos::client::v1::Plugins::NewStub(channel)->Set(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Set: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::PluginsDeleteResponse delete_(grpc::ClientContext& context, const ::harmos::client::v1::PluginsDeleteRequest& request) {
    ::harmos::client::v1::PluginsDeleteResponse response;
    const auto status = ::harmos::client::v1::Plugins::NewStub(channel)->Delete(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Delete: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::PluginsRestartResponse restart(grpc::ClientContext& context, const ::harmos::client::v1::PluginsRestartRequest& request) {
    ::harmos::client::v1::PluginsRestartResponse response;
    const auto status = ::harmos::client::v1::Plugins::NewStub(channel)->Restart(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Restart: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::PluginsReplaceResponse replace(grpc::ClientContext& context, const ::harmos::client::v1::PluginsReplaceRequest& request) {
    ::harmos::client::v1::PluginsReplaceResponse response;
    const auto status = ::harmos::client::v1::Plugins::NewStub(channel)->Replace(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Replace: ") + status.error_message());
    return response;
  }
};
struct SchemasResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit SchemasResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::SchemasListResponse list(grpc::ClientContext& context, const ::harmos::client::v1::SchemasListRequest& request) {
    ::harmos::client::v1::SchemasListResponse response;
    const auto status = ::harmos::client::v1::Schemas::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
};
struct StatusResource {
  std::shared_ptr<grpc::Channel> channel;
  StatusDiagnosticsResource diagnostics;
  StatusLogsResource logs;
  StatusTraceEventsResource trace_events;
  explicit StatusResource(std::shared_ptr<grpc::Channel> channel) : channel(channel), diagnostics(channel), logs(channel), trace_events(channel) {}
  ::harmos::client::v1::StatusGetResponse get(grpc::ClientContext& context, const ::harmos::client::v1::StatusGetRequest& request) {
    ::harmos::client::v1::StatusGetResponse response;
    const auto status = ::harmos::client::v1::Status::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
  std::unique_ptr<grpc::ClientReader<::harmos::client::v1::StatusWatchResponse>> watch(grpc::ClientContext& context, const ::harmos::client::v1::StatusWatchRequest& request) {
    return ::harmos::client::v1::Status::NewStub(channel)->Watch(&context, request);
  }
};
struct StreamsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StreamsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::StreamsListResponse list(grpc::ClientContext& context, const ::harmos::client::v1::StreamsListRequest& request) {
    ::harmos::client::v1::StreamsListResponse response;
    const auto status = ::harmos::client::v1::Streams::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
};
struct TimeResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit TimeResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::v1::TimeGetResponse get(grpc::ClientContext& context, const ::harmos::client::v1::TimeGetRequest& request) {
    ::harmos::client::v1::TimeGetResponse response;
    const auto status = ::harmos::client::v1::Time::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
  ::harmos::client::v1::TimeSleepResponse sleep(grpc::ClientContext& context, const ::harmos::client::v1::TimeSleepRequest& request) {
    ::harmos::client::v1::TimeSleepResponse response;
    const auto status = ::harmos::client::v1::Time::NewStub(channel)->Sleep(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Sleep: ") + status.error_message());
    return response;
  }
};
struct Client {
  AccessResource access;
  InvocationsResource invocations;
  OperationsResource operations;
  PluginsResource plugins;
  SchemasResource schemas;
  StatusResource status;
  StreamsResource streams;
  TimeResource time;
  explicit Client(std::shared_ptr<grpc::Channel> channel) : access(channel), invocations(channel), operations(channel), plugins(channel), schemas(channel), status(channel), streams(channel), time(channel) {}
  static Client connect(std::string address, const ::harmos::v1::StorageTransferSettings& transfer = {}, std::optional<int> message_bytes = {}) {
    const auto maximum = ::harmos::transfer_message_bytes(transfer, message_bytes);
    grpc::ChannelArguments arguments; arguments.SetMaxSendMessageSize(maximum); arguments.SetMaxReceiveMessageSize(maximum);
    if (address.starts_with("http://")) address.erase(0, 7);
    return Client(grpc::CreateCustomChannel(address, grpc::InsecureChannelCredentials(), arguments));
  }
};
}
#pragma pop_macro("Watch")
#pragma pop_macro("Sleep")
#pragma pop_macro("Set")
#pragma pop_macro("Restart")
#pragma pop_macro("Replace")
#pragma pop_macro("List")
#pragma pop_macro("Invoke")
#pragma pop_macro("Get")
#pragma pop_macro("Delete")
#pragma pop_macro("Create")
#pragma pop_macro("Cancel")
