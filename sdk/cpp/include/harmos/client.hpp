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
  ::harmos::v1::StorageCreateFileProductionResponse create_file_production(const ::harmos::v1::StorageCreateFileProductionRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageCreateFileProductionResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->CreateFileProduction(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("CreateFileProduction: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageGetFileProductionResponse get_file_production(const ::harmos::v1::StorageGetFileProductionRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageGetFileProductionResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->GetFileProduction(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("GetFileProduction: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageAppendFileBytesResponse append_file_bytes(const ::harmos::v1::StorageAppendFileBytesRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageAppendFileBytesResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->AppendFileBytes(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("AppendFileBytes: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageCreateFileWriterResponse create_file_writer(const ::harmos::v1::StorageCreateFileWriterRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageCreateFileWriterResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->CreateFileWriter(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("CreateFileWriter: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StoragePublishFileProductionResponse publish_file_production(const ::harmos::v1::StoragePublishFileProductionRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StoragePublishFileProductionResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->PublishFileProduction(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("PublishFileProduction: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageDeleteFileProductionResponse delete_file_production(const ::harmos::v1::StorageDeleteFileProductionRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageDeleteFileProductionResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->DeleteFileProduction(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("DeleteFileProduction: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageGetFileBytesResponse get_file_bytes(const ::harmos::v1::StorageGetFileBytesRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageGetFileBytesResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->GetFileBytes(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("GetFileBytes: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageCapabilitiesResponse capabilities(const ::harmos::v1::StorageCapabilitiesRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageCapabilitiesResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->Capabilities(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Capabilities: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageCreateSnapshotResponse create_snapshot(const ::harmos::v1::StorageCreateSnapshotRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageCreateSnapshotResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->CreateSnapshot(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("CreateSnapshot: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageDeleteSnapshotResponse delete_snapshot(const ::harmos::v1::StorageDeleteSnapshotRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageDeleteSnapshotResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->DeleteSnapshot(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("DeleteSnapshot: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageListColumnRowsResponse list_column_rows(const ::harmos::v1::StorageListColumnRowsRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageListColumnRowsResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->ListColumnRows(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("ListColumnRows: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageListIndexEntriesResponse list_index_entries(const ::harmos::v1::StorageListIndexEntriesRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageListIndexEntriesResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->ListIndexEntries(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("ListIndexEntries: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageSetIndexDefinitionResponse set_index_definition(const ::harmos::v1::StorageSetIndexDefinitionRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageSetIndexDefinitionResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->SetIndexDefinition(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("SetIndexDefinition: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageSetProjectionDefinitionResponse set_projection_definition(const ::harmos::v1::StorageSetProjectionDefinitionRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageSetProjectionDefinitionResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->SetProjectionDefinition(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("SetProjectionDefinition: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageAdvanceProjectionResponse advance_projection(const ::harmos::v1::StorageAdvanceProjectionRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageAdvanceProjectionResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->AdvanceProjection(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("AdvanceProjection: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageListProjectionRowsResponse list_projection_rows(const ::harmos::v1::StorageListProjectionRowsRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageListProjectionRowsResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->ListProjectionRows(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("ListProjectionRows: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageGetProjectionStatusResponse get_projection_status(const ::harmos::v1::StorageGetProjectionStatusRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageGetProjectionStatusResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->GetProjectionStatus(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("GetProjectionStatus: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageSetObjectResponse set_object(const ::harmos::v1::StorageSetObjectRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageSetObjectResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->SetObject(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("SetObject: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageListRowsResponse list_rows(const ::harmos::v1::StorageListRowsRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageListRowsResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->ListRows(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("ListRows: ") + status.error_message());
    return response;
  }
  ClientWatch<::harmos::v1::StorageWatchRowsResponse> watch_rows(const ::harmos::v1::StorageWatchRowsRequest& request) {
    auto context = std::make_unique<grpc::ClientContext>();
    auto reader = ::harmos::v1::Storage::NewStub(channel)->WatchRows(context.get(), request);
    return {std::move(context), std::move(reader)};
  }
  ::harmos::v1::StorageAppendRecordsResponse append_records(const ::harmos::v1::StorageAppendRecordsRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageAppendRecordsResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->AppendRecords(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("AppendRecords: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageAppendSamplesResponse append_samples(const ::harmos::v1::StorageAppendSamplesRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageAppendSamplesResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->AppendSamples(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("AppendSamples: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageAppendObjectRevisionsResponse append_object_revisions(const ::harmos::v1::StorageAppendObjectRevisionsRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageAppendObjectRevisionsResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->AppendObjectRevisions(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("AppendObjectRevisions: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageGetFileHashResponse get_file_hash(const ::harmos::v1::StorageGetFileHashRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageGetFileHashResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->GetFileHash(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("GetFileHash: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageCaptureFileResponse capture_file(const ::harmos::v1::StorageCaptureFileRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageCaptureFileResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->CaptureFile(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("CaptureFile: ") + status.error_message());
    return response;
  }
  ClientWatch<::harmos::v1::StorageWatchFileBytesResponse> watch_file_bytes(const ::harmos::v1::StorageWatchFileBytesRequest& request) {
    auto context = std::make_unique<grpc::ClientContext>();
    auto reader = ::harmos::v1::Storage::NewStub(channel)->WatchFileBytes(context.get(), request);
    return {std::move(context), std::move(reader)};
  }
  ClientWatch<::harmos::v1::StorageWatchFileProductionResponse> watch_file_production(const ::harmos::v1::StorageWatchFileProductionRequest& request) {
    auto context = std::make_unique<grpc::ClientContext>();
    auto reader = ::harmos::v1::Storage::NewStub(channel)->WatchFileProduction(context.get(), request);
    return {std::move(context), std::move(reader)};
  }
  ::harmos::v1::StorageListDirectoryEntriesResponse list_directory_entries(const ::harmos::v1::StorageListDirectoryEntriesRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageListDirectoryEntriesResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->ListDirectoryEntries(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("ListDirectoryEntries: ") + status.error_message());
    return response;
  }
  ClientWatch<::harmos::v1::StorageWatchDirectoryEntriesResponse> watch_directory_entries(const ::harmos::v1::StorageWatchDirectoryEntriesRequest& request) {
    auto context = std::make_unique<grpc::ClientContext>();
    auto reader = ::harmos::v1::Storage::NewStub(channel)->WatchDirectoryEntries(context.get(), request);
    return {std::move(context), std::move(reader)};
  }
  ::harmos::v1::StorageGetIndexDefinitionResponse get_index_definition(const ::harmos::v1::StorageGetIndexDefinitionRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageGetIndexDefinitionResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->GetIndexDefinition(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("GetIndexDefinition: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageGetProjectionDefinitionResponse get_projection_definition(const ::harmos::v1::StorageGetProjectionDefinitionRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageGetProjectionDefinitionResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->GetProjectionDefinition(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("GetProjectionDefinition: ") + status.error_message());
    return response;
  }
  ::harmos::v1::StorageGetObjectResponse get_object(const ::harmos::v1::StorageGetObjectRequest& request) {
    grpc::ClientContext context;
    ::harmos::v1::StorageGetObjectResponse response;
    const auto status = ::harmos::v1::Storage::NewStub(channel)->GetObject(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("GetObject: ") + status.error_message());
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
