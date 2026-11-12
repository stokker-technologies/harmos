// @generated from the protocol descriptor table; do not edit.
#pragma once
#include <grpcpp/grpcpp.h>
#include <memory>
#include <stdexcept>
#include "harmos/v1/access/certificates.grpc.pb.h"
#include "harmos/v1/access/listener.grpc.pb.h"
#include "harmos/v1/access/pairings.grpc.pb.h"
#include "harmos/v1/catalog_entries.grpc.pb.h"
#include "harmos/v1/invocations.grpc.pb.h"
#include "harmos/v1/plugins.grpc.pb.h"
#include "harmos/v1/status.grpc.pb.h"
#include "harmos/v1/status/diagnostics.grpc.pb.h"
#include "harmos/v1/status/logs.grpc.pb.h"
#include "harmos/v1/status/trace_events.grpc.pb.h"
#include "harmos/v1/storage.grpc.pb.h"
#include "harmos/v1/time.grpc.pb.h"
namespace harmos {
template<class Response> struct ClientWatch {
  std::unique_ptr<grpc::ClientContext> context;
  std::unique_ptr<grpc::ClientReader<Response>> reader;
  ~ClientWatch() { if (context) context->TryCancel(); }
};
struct AccessCertificatesResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit AccessCertificatesResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::v1::access::CertificatesCreateResponse create(const ::harmos::v1::access::CertificatesCreateRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::access::CertificatesCreateResponse response;
    const auto status = ::harmos::v1::access::Certificates::NewStub(channel)->Create(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Create: ") + status.error_message());
    return response;
  }
};
struct AccessListenerResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit AccessListenerResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::v1::access::ListenerGetResponse get(const ::harmos::v1::access::ListenerGetRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::access::ListenerGetResponse response;
    const auto status = ::harmos::v1::access::Listener::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
  ::harmos::v1::access::ListenerSetResponse set(const ::harmos::v1::access::ListenerSetRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::access::ListenerSetResponse response;
    const auto status = ::harmos::v1::access::Listener::NewStub(channel)->Set(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Set: ") + status.error_message());
    return response;
  }
};
struct AccessPairingsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit AccessPairingsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::v1::access::PairingsCreateResponse create(const ::harmos::v1::access::PairingsCreateRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::access::PairingsCreateResponse response;
    const auto status = ::harmos::v1::access::Pairings::NewStub(channel)->Create(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Create: ") + status.error_message());
    return response;
  }
  ::harmos::v1::access::PairingsListResponse list(const ::harmos::v1::access::PairingsListRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::access::PairingsListResponse response;
    const auto status = ::harmos::v1::access::Pairings::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
  ::harmos::v1::access::PairingsDeleteResponse delete(const ::harmos::v1::access::PairingsDeleteRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::access::PairingsDeleteResponse response;
    const auto status = ::harmos::v1::access::Pairings::NewStub(channel)->Delete(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Delete: ") + status.error_message());
    return response;
  }
};
struct StatusDiagnosticsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StatusDiagnosticsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::v1::status::DiagnosticsGetResponse get(const ::harmos::v1::status::DiagnosticsGetRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::status::DiagnosticsGetResponse response;
    const auto status = ::harmos::v1::status::Diagnostics::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
};
struct StatusLogsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StatusLogsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::v1::status::LogsListResponse list(const ::harmos::v1::status::LogsListRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::status::LogsListResponse response;
    const auto status = ::harmos::v1::status::Logs::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
  ClientWatch<::harmos::v1::status::LogsWatchResponse> watch(const ::harmos::v1::status::LogsWatchRequest& request) {
    auto context = std::make_unique<grpc::ClientContext>();
    auto reader = ::harmos::v1::status::Logs::NewStub(channel)->Watch(context.get(), request);
    return {std::move(context), std::move(reader)};
  }
};
struct StatusTraceEventsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StatusTraceEventsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::v1::status::TraceEventsListResponse list(const ::harmos::v1::status::TraceEventsListRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::status::TraceEventsListResponse response;
    const auto status = ::harmos::v1::status::TraceEvents::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
  ClientWatch<::harmos::v1::status::TraceEventsWatchResponse> watch(const ::harmos::v1::status::TraceEventsWatchRequest& request) {
    auto context = std::make_unique<grpc::ClientContext>();
    auto reader = ::harmos::v1::status::TraceEvents::NewStub(channel)->Watch(context.get(), request);
    return {std::move(context), std::move(reader)};
  }
};
struct AccessResource {
  AccessCertificatesResource certificates;
  AccessListenerResource listener;
  AccessPairingsResource pairings;
  explicit AccessResource(std::shared_ptr<grpc::Channel> channel) : certificates(channel), listener(channel), pairings(channel) {}
};
struct CatalogEntriesResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit CatalogEntriesResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::v1::CatalogEntriesListResponse list(const ::harmos::v1::CatalogEntriesListRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::CatalogEntriesListResponse response;
    const auto status = ::harmos::v1::CatalogEntries::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
};
struct InvocationsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit InvocationsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::v1::InvocationsInvokeResponse invoke(const ::harmos::v1::InvocationsInvokeRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::InvocationsInvokeResponse response;
    const auto status = ::harmos::v1::Invocations::NewStub(channel)->Invoke(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Invoke: ") + status.error_message());
    return response;
  }
  ::harmos::v1::InvocationsGetResponse get(const ::harmos::v1::InvocationsGetRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::InvocationsGetResponse response;
    const auto status = ::harmos::v1::Invocations::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
  ::harmos::v1::InvocationsCancelResponse cancel(const ::harmos::v1::InvocationsCancelRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::InvocationsCancelResponse response;
    const auto status = ::harmos::v1::Invocations::NewStub(channel)->Cancel(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Cancel: ") + status.error_message());
    return response;
  }
  ClientWatch<::harmos::v1::InvocationsWatchResponse> watch(const ::harmos::v1::InvocationsWatchRequest& request) {
    auto context = std::make_unique<grpc::ClientContext>();
    auto reader = ::harmos::v1::Invocations::NewStub(channel)->Watch(context.get(), request);
    return {std::move(context), std::move(reader)};
  }
};
struct PluginsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit PluginsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::v1::PluginsGetResponse get(const ::harmos::v1::PluginsGetRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::PluginsGetResponse response;
    const auto status = ::harmos::v1::Plugins::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
  ::harmos::v1::PluginsListResponse list(const ::harmos::v1::PluginsListRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::PluginsListResponse response;
    const auto status = ::harmos::v1::Plugins::NewStub(channel)->List(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("List: ") + status.error_message());
    return response;
  }
  ClientWatch<::harmos::v1::PluginsWatchResponse> watch(const ::harmos::v1::PluginsWatchRequest& request) {
    auto context = std::make_unique<grpc::ClientContext>();
    auto reader = ::harmos::v1::Plugins::NewStub(channel)->Watch(context.get(), request);
    return {std::move(context), std::move(reader)};
  }
  ::harmos::v1::PluginsCreateResponse create(const ::harmos::v1::PluginsCreateRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::PluginsCreateResponse response;
    const auto status = ::harmos::v1::Plugins::NewStub(channel)->Create(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Create: ") + status.error_message());
    return response;
  }
  ::harmos::v1::PluginsSetResponse set(const ::harmos::v1::PluginsSetRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::PluginsSetResponse response;
    const auto status = ::harmos::v1::Plugins::NewStub(channel)->Set(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Set: ") + status.error_message());
    return response;
  }
  ::harmos::v1::PluginsDeleteResponse delete(const ::harmos::v1::PluginsDeleteRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::PluginsDeleteResponse response;
    const auto status = ::harmos::v1::Plugins::NewStub(channel)->Delete(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Delete: ") + status.error_message());
    return response;
  }
  ::harmos::v1::PluginsRestartResponse restart(const ::harmos::v1::PluginsRestartRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::PluginsRestartResponse response;
    const auto status = ::harmos::v1::Plugins::NewStub(channel)->Restart(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Restart: ") + status.error_message());
    return response;
  }
};
struct StatusResource {
  std::shared_ptr<grpc::Channel> channel;
  StatusDiagnosticsResource diagnostics;
  StatusLogsResource logs;
  StatusTraceEventsResource trace_events;
  explicit StatusResource(std::shared_ptr<grpc::Channel> channel) : channel(channel), diagnostics(channel), logs(channel), trace_events(channel) {}
  ::harmos::v1::StatusGetResponse get(const ::harmos::v1::StatusGetRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StatusGetResponse response;
    const auto status = ::harmos::v1::Status::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
  ClientWatch<::harmos::v1::StatusWatchResponse> watch(const ::harmos::v1::StatusWatchRequest& request) {
    auto context = std::make_unique<grpc::ClientContext>();
    auto reader = ::harmos::v1::Status::NewStub(channel)->Watch(context.get(), request);
    return {std::move(context), std::move(reader)};
  }
};
struct StorageResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StorageResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::storage::v1::StorageCapabilities capabilities(const ::harmos::storage::v1::Capabilities& request) {
    grpc::ClientContext context;
    ::harmos::storage::v1::StorageCapabilities response;
    const auto status = ::harmos::storage::v1::Storage::NewStub(channel)->Capabilities(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Capabilities: ") + status.error_message());
    return response;
  }
  ::harmos::storage::v1::SavedSnapshot open_snapshot(const ::harmos::storage::v1::OpenSnapshot& request) {
    grpc::ClientContext context;
    ::harmos::storage::v1::SavedSnapshot response;
    const auto status = ::harmos::storage::v1::Storage::NewStub(channel)->OpenSnapshot(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("OpenSnapshot: ") + status.error_message());
    return response;
  }
  ::harmos::storage::v1::Empty release_snapshot(const ::harmos::storage::v1::ReleaseSnapshot& request) {
    grpc::ClientContext context;
    ::harmos::storage::v1::Empty response;
    const auto status = ::harmos::storage::v1::Storage::NewStub(channel)->ReleaseSnapshot(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("ReleaseSnapshot: ") + status.error_message());
    return response;
  }
  ::harmos::storage::v1::ArrowPage scan_arrow(const ::harmos::storage::v1::ScanArrow& request) {
    grpc::ClientContext context;
    ::harmos::storage::v1::ArrowPage response;
    const auto status = ::harmos::storage::v1::Storage::NewStub(channel)->ScanArrow(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("ScanArrow: ") + status.error_message());
    return response;
  }
  ::harmos::storage::v1::IndexPage scan_index(const ::harmos::storage::v1::ScanIndex& request) {
    grpc::ClientContext context;
    ::harmos::storage::v1::IndexPage response;
    const auto status = ::harmos::storage::v1::Storage::NewStub(channel)->ScanIndex(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("ScanIndex: ") + status.error_message());
    return response;
  }
  ::harmos::storage::v1::DefinitionRevision register_index(const ::harmos::storage::v1::RegisterIndex& request) {
    grpc::ClientContext context;
    ::harmos::storage::v1::DefinitionRevision response;
    const auto status = ::harmos::storage::v1::Storage::NewStub(channel)->RegisterIndex(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("RegisterIndex: ") + status.error_message());
    return response;
  }
  ::harmos::storage::v1::DefinitionRevision register_projection(const ::harmos::storage::v1::RegisterProjection& request) {
    grpc::ClientContext context;
    ::harmos::storage::v1::DefinitionRevision response;
    const auto status = ::harmos::storage::v1::Storage::NewStub(channel)->RegisterProjection(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("RegisterProjection: ") + status.error_message());
    return response;
  }
  ::harmos::storage::v1::DefinitionRevision advance_projection(const ::harmos::storage::v1::AdvanceProjection& request) {
    grpc::ClientContext context;
    ::harmos::storage::v1::DefinitionRevision response;
    const auto status = ::harmos::storage::v1::Storage::NewStub(channel)->AdvanceProjection(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("AdvanceProjection: ") + status.error_message());
    return response;
  }
  ::harmos::storage::v1::ProjectionPage read_projection(const ::harmos::storage::v1::ReadProjection& request) {
    grpc::ClientContext context;
    ::harmos::storage::v1::ProjectionPage response;
    const auto status = ::harmos::storage::v1::Storage::NewStub(channel)->ReadProjection(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("ReadProjection: ") + status.error_message());
    return response;
  }
  ::harmos::storage::v1::ProjectionState inspect_projection(const ::harmos::storage::v1::InspectProjection& request) {
    grpc::ClientContext context;
    ::harmos::storage::v1::ProjectionState response;
    const auto status = ::harmos::storage::v1::Storage::NewStub(channel)->InspectProjection(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("InspectProjection: ") + status.error_message());
    return response;
  }
  ::harmos::storage::v1::CompareExchangeOutcome compare_exchange_object(const ::harmos::storage::v1::CompareExchangeObject& request) {
    grpc::ClientContext context;
    ::harmos::storage::v1::CompareExchangeOutcome response;
    const auto status = ::harmos::storage::v1::Storage::NewStub(channel)->CompareExchangeObject(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("CompareExchangeObject: ") + status.error_message());
    return response;
  }
};
struct TimeResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit TimeResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::v1::TimeGetResponse get(const ::harmos::v1::TimeGetRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::TimeGetResponse response;
    const auto status = ::harmos::v1::Time::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
  ::harmos::v1::TimeSleepResponse sleep(const ::harmos::v1::TimeSleepRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::TimeSleepResponse response;
    const auto status = ::harmos::v1::Time::NewStub(channel)->Sleep(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Sleep: ") + status.error_message());
    return response;
  }
};
struct Client {
  AccessResource access;
  CatalogEntriesResource catalog_entries;
  InvocationsResource invocations;
  PluginsResource plugins;
  StatusResource status;
  StorageResource storage;
  TimeResource time;
  explicit Client(std::shared_ptr<grpc::Channel> channel) : access(channel), catalog_entries(channel), invocations(channel), plugins(channel), status(channel), storage(channel), time(channel) {}
};
}
