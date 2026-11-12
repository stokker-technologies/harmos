// @generated from the protocol descriptor table; do not edit.
#pragma once
#include <grpcpp/grpcpp.h>
#include <memory>
#include <stdexcept>
#include "harmos/v1/catalog_entries.grpc.pb.h"
#include "harmos/v1/invocations.grpc.pb.h"
#include "harmos/v1/storage.grpc.pb.h"
#include "harmos/v1/time.grpc.pb.h"
namespace harmos {
template<class Response> struct ClientWatch {
  std::unique_ptr<grpc::ClientContext> context;
  std::unique_ptr<grpc::ClientReader<Response>> reader;
  ~ClientWatch() { if (context) context->TryCancel(); }
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
  CatalogEntriesResource catalog_entries;
  InvocationsResource invocations;
  StorageResource storage;
  TimeResource time;
  explicit Client(std::shared_ptr<grpc::Channel> channel) : catalog_entries(channel), invocations(channel), storage(channel), time(channel) {}
};
}
