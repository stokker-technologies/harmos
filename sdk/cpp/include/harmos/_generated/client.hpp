// @generated from protocol descriptors; do not edit.
#pragma once
#include "client_wire.hpp"
#include "harmos/client_transport.hpp"
#include "harmos/messages.hpp"
namespace harmos {
namespace client {
struct AccessCertificatesCreateOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct AccessListenerGetOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct AccessListenerSetOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct AccessPairingsCreateOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct AccessPairingsListOptions {
  std::optional<::harmos::Position> at;
  std::shared_ptr<grpc::ClientContext> context;
};
struct AccessPairingsDeleteOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct InvocationsCreateOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct InvocationsGetOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct InvocationsCancelOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct InvocationsWatchOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct OperationsListOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct OperationsGetOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct PluginsSettingsListOptions {
  std::optional<std::string> name;
  std::optional<::harmos::ReadCut> at;
  std::shared_ptr<grpc::ClientContext> context;
};
struct PluginsSettingsSetOptions {
  std::optional<std::string> request_id;
  std::shared_ptr<grpc::ClientContext> context;
};
struct PluginsGetOptions {
  std::optional<::harmos::ReadCut> at;
  std::shared_ptr<grpc::ClientContext> context;
};
struct PluginsListOptions {
  std::optional<::harmos::ReadCut> at;
  std::shared_ptr<grpc::ClientContext> context;
};
struct PluginsWatchOptions {
  std::optional<std::string> name;
  std::optional<::harmos::ReadCut> at;
  std::shared_ptr<grpc::ClientContext> context;
};
struct PluginsCreateOptions {
  std::optional<std::string> request_id;
  std::shared_ptr<grpc::ClientContext> context;
};
struct PluginsSetOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct PluginsDeleteOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct PluginsRestartOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct PluginsReplaceOptions {
  std::optional<std::string> request_id;
  std::shared_ptr<grpc::ClientContext> context;
};
struct SchemasListOptions {
  std::optional<std::string> schema;
  std::shared_ptr<grpc::ClientContext> context;
};
struct StatusDiagnosticsGetOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct StatusLogsListOptions {
  std::optional<::harmos::Position> at;
  std::optional<::harmos::PageRequest> page;
  std::shared_ptr<grpc::ClientContext> context;
};
struct StatusLogsWatchOptions {
  std::optional<::harmos::Position> at;
  std::shared_ptr<grpc::ClientContext> context;
};
struct StatusTraceEventsListOptions {
  std::optional<::harmos::Position> at;
  std::optional<::harmos::PageRequest> page;
  std::shared_ptr<grpc::ClientContext> context;
};
struct StatusTraceEventsWatchOptions {
  std::optional<::harmos::Position> at;
  std::shared_ptr<grpc::ClientContext> context;
};
struct StatusGetOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct StatusWatchOptions {
  std::optional<::harmos::Position> at;
  std::shared_ptr<grpc::ClientContext> context;
};
struct StreamsListOptions {
  std::optional<std::string> schema;
  std::shared_ptr<grpc::ClientContext> context;
};
struct TimeGetOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
struct TimeSleepOptions {
  std::shared_ptr<grpc::ClientContext> context;
};
} // namespace client
struct AccessCertificatesResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit AccessCertificatesResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  std::string create(client::AccessCertificatesCreateOptions options = {}) {
    detail::AccessCertificatesCreateRequest request;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::AccessCertificatesCreateResponse response;
    const auto status = detail::AccessCertificatesCreateService::NewStub(channel)->Create(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.certificates.create: " + status.error_message());
    }
    return response.certificate_pem();
  }
};
struct AccessListenerResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit AccessListenerResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::ListenerGetResponse get(client::AccessListenerGetOptions options = {}) {
    detail::AccessListenerGetRequest request;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::AccessListenerGetResponse response;
    const auto status = detail::AccessListenerGetService::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.listener.get: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::ListenerSetResponse set(const ::harmos::client::ListenerValue &value, const std::string &expected_revision,
                                            client::AccessListenerSetOptions options = {}) {
    detail::AccessListenerSetRequest request;
    *request.mutable_value() = value;
    request.set_expected_revision(expected_revision);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::AccessListenerSetResponse response;
    const auto status = detail::AccessListenerSetService::NewStub(channel)->Set(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.listener.set: " + status.error_message());
    }
    return response;
  }
};
struct AccessPairingsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit AccessPairingsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::PairingsCreateResponse create(const std::string &label, client::AccessPairingsCreateOptions options = {}) {
    detail::AccessPairingsCreateRequest request;
    request.set_label(label);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::AccessPairingsCreateResponse response;
    const auto status = detail::AccessPairingsCreateService::NewStub(channel)->Create(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.pairings.create: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::PairingsListResponse list(client::AccessPairingsListOptions options = {}) {
    detail::AccessPairingsListRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::AccessPairingsListResponse response;
    const auto status = detail::AccessPairingsListService::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.pairings.list: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::PairingsDeleteResponse delete_(const std::string &name, client::AccessPairingsDeleteOptions options = {}) {
    detail::AccessPairingsDeleteRequest request;
    request.set_name(name);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::AccessPairingsDeleteResponse response;
    const auto status = detail::AccessPairingsDeleteService::NewStub(channel)->Delete(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.pairings.delete: " + status.error_message());
    }
    return response;
  }
};
struct PluginsSettingsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit PluginsSettingsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::SettingsListResponse list(client::PluginsSettingsListOptions options = {}) {
    detail::PluginsSettingsListRequest request;
    if (options.name) {
      request.set_name(*options.name);
    }
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::PluginsSettingsListResponse response;
    const auto status = detail::PluginsSettingsListService::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.settings.list: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::SettingsSetResponse set(const std::string &name, const ::harmos::SettingValues &value,
                                            const std::string &expected_revision, client::PluginsSettingsSetOptions options = {}) {
    detail::PluginsSettingsSetRequest request;
    request.set_name(name);
    *request.mutable_value() = value;
    request.set_expected_revision(expected_revision);
    request.set_request_id(options.request_id ? *options.request_id : ::harmos::detail::request_id());
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::PluginsSettingsSetResponse response;
    const auto status = detail::PluginsSettingsSetService::NewStub(channel)->Set(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.settings.set: " + status.error_message());
    }
    return response;
  }
};
struct StatusDiagnosticsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StatusDiagnosticsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::DiagnosticsGetResponse get(const std::string &name, const std::uint64_t &offset, const std::uint64_t &max_bytes,
                                               client::StatusDiagnosticsGetOptions options = {}) {
    detail::StatusDiagnosticsGetRequest request;
    request.set_name(name);
    request.set_offset(offset);
    request.set_max_bytes(max_bytes);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::StatusDiagnosticsGetResponse response;
    const auto status = detail::StatusDiagnosticsGetService::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call status.diagnostics.get: " + status.error_message());
    }
    return response;
  }
};
struct StatusLogsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StatusLogsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::LogsListResponse list(client::StatusLogsListOptions options = {}) {
    detail::StatusLogsListRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    if (options.page) {
      *request.mutable_page() = *options.page;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::StatusLogsListResponse response;
    const auto status = detail::StatusLogsListService::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call status.logs.list: " + status.error_message());
    }
    return response;
  }
  ::harmos::ClientStream<::harmos::client::LogsWatchResponse> watch(const std::uint64_t &max_bytes, const std::uint32_t &max_events,
                                                                    client::StatusLogsWatchOptions options = {}) {
    detail::StatusLogsWatchRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    request.set_max_bytes(max_bytes);
    request.set_max_events(max_events);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    auto reader = std::shared_ptr<grpc::ClientReader<detail::StatusLogsWatchResponse>>(
        detail::StatusLogsWatchService::NewStub(channel)->Watch(context.get(), request));
    return {context, [reader]() -> std::optional<::harmos::client::LogsWatchResponse> {
              detail::StatusLogsWatchResponse response;
              if (!reader->Read(&response)) {
                const auto status = reader->Finish();
                if (!status.ok()) {
                  throw std::runtime_error("Read status.logs.watch: " + status.error_message());
                }
                return {};
              }
              return response;
            }};
  }
};
struct StatusTraceEventsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StatusTraceEventsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::harmos::client::TraceEventsListResponse list(client::StatusTraceEventsListOptions options = {}) {
    detail::StatusTraceEventsListRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    if (options.page) {
      *request.mutable_page() = *options.page;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::StatusTraceEventsListResponse response;
    const auto status = detail::StatusTraceEventsListService::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call status.trace_events.list: " + status.error_message());
    }
    return response;
  }
  ::harmos::ClientStream<::harmos::client::TraceEventsWatchResponse> watch(const std::uint64_t &max_bytes, const std::uint32_t &max_events,
                                                                           client::StatusTraceEventsWatchOptions options = {}) {
    detail::StatusTraceEventsWatchRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    request.set_max_bytes(max_bytes);
    request.set_max_events(max_events);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    auto reader = std::shared_ptr<grpc::ClientReader<detail::StatusTraceEventsWatchResponse>>(
        detail::StatusTraceEventsWatchService::NewStub(channel)->Watch(context.get(), request));
    return {context, [reader]() -> std::optional<::harmos::client::TraceEventsWatchResponse> {
              detail::StatusTraceEventsWatchResponse response;
              if (!reader->Read(&response)) {
                const auto status = reader->Finish();
                if (!status.ok()) {
                  throw std::runtime_error("Read status.trace_events.watch: " + status.error_message());
                }
                return {};
              }
              return response;
            }};
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
  std::string create(const std::string &operation, const std::string &fingerprint, const std::string &payload,
                     client::InvocationsCreateOptions options = {}) {
    detail::InvocationsCreateRequest request;
    request.set_operation(operation);
    request.set_fingerprint(fingerprint);
    request.set_payload(payload);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::InvocationsCreateResponse response;
    const auto status = detail::InvocationsCreateService::NewStub(channel)->Create(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call invocations.create: " + status.error_message());
    }
    return response.payload();
  }
  ::harmos::InstanceState get(const std::uint64_t &instance, client::InvocationsGetOptions options = {}) {
    detail::InvocationsGetRequest request;
    request.set_instance(instance);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::InvocationsGetResponse response;
    const auto status = detail::InvocationsGetService::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call invocations.get: " + status.error_message());
    }
    if (!response.has_invocation()) {
      throw std::runtime_error("Read invocations.get: Missing Payload");
    }
    return std::move(*response.mutable_invocation());
  }
  ::harmos::InstanceState cancel(const std::uint64_t &instance, const std::string &reason, client::InvocationsCancelOptions options = {}) {
    detail::InvocationsCancelRequest request;
    request.set_instance(instance);
    request.set_reason(reason);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::InvocationsCancelResponse response;
    const auto status = detail::InvocationsCancelService::NewStub(channel)->Cancel(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call invocations.cancel: " + status.error_message());
    }
    if (!response.has_invocation()) {
      throw std::runtime_error("Read invocations.cancel: Missing Payload");
    }
    return std::move(*response.mutable_invocation());
  }
  ::harmos::ClientStream<::harmos::InstanceState> watch(const std::uint64_t &instance, client::InvocationsWatchOptions options = {}) {
    detail::InvocationsWatchRequest request;
    request.set_instance(instance);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    auto reader = std::shared_ptr<grpc::ClientReader<detail::InvocationsWatchResponse>>(
        detail::InvocationsWatchService::NewStub(channel)->Watch(context.get(), request));
    return {context, [reader]() -> std::optional<::harmos::InstanceState> {
              detail::InvocationsWatchResponse response;
              if (!reader->Read(&response)) {
                const auto status = reader->Finish();
                if (!status.ok()) {
                  throw std::runtime_error("Read invocations.watch: " + status.error_message());
                }
                return {};
              }
              if (!response.has_invocation()) {
                throw std::runtime_error("Read invocations.watch: Missing Payload");
              }
              return std::move(*response.mutable_invocation());
            }};
  }
};
struct OperationsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit OperationsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  std::vector<::harmos::DiscoveredOperation> list(client::OperationsListOptions options = {}) {
    detail::OperationsListRequest request;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::OperationsListResponse response;
    const auto status = detail::OperationsListService::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call operations.list: " + status.error_message());
    }
    return std::vector<::harmos::DiscoveredOperation>(response.operations().begin(), response.operations().end());
  }
  ::harmos::DiscoveredOperation get(const std::string &id, client::OperationsGetOptions options = {}) {
    detail::OperationsGetRequest request;
    request.set_id(id);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::OperationsGetResponse response;
    const auto status = detail::OperationsGetService::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call operations.get: " + status.error_message());
    }
    if (!response.has_operation()) {
      throw std::runtime_error("Read operations.get: Missing Payload");
    }
    return std::move(*response.mutable_operation());
  }
};
struct PluginsResource {
  std::shared_ptr<grpc::Channel> channel;
  PluginsSettingsResource settings;
  explicit PluginsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel), settings(channel) {}
  ::harmos::client::PluginsGetResponse get(const std::string &name, client::PluginsGetOptions options = {}) {
    detail::PluginsGetRequest request;
    request.set_name(name);
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::PluginsGetResponse response;
    const auto status = detail::PluginsGetService::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.get: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::PluginsListResponse list(client::PluginsListOptions options = {}) {
    detail::PluginsListRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::PluginsListResponse response;
    const auto status = detail::PluginsListService::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.list: " + status.error_message());
    }
    return response;
  }
  ::harmos::ClientStream<::harmos::client::PluginsWatchResponse> watch(client::PluginsWatchOptions options = {}) {
    detail::PluginsWatchRequest request;
    if (options.name) {
      request.set_name(*options.name);
    }
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    auto reader = std::shared_ptr<grpc::ClientReader<detail::PluginsWatchResponse>>(
        detail::PluginsWatchService::NewStub(channel)->Watch(context.get(), request));
    return {context, [reader]() -> std::optional<::harmos::client::PluginsWatchResponse> {
              detail::PluginsWatchResponse response;
              if (!reader->Read(&response)) {
                const auto status = reader->Finish();
                if (!status.ok()) {
                  throw std::runtime_error("Read plugins.watch: " + status.error_message());
                }
                return {};
              }
              return response;
            }};
  }
  ::harmos::client::PluginsCreateResponse create(const std::string &name, const ::harmos::client::PluginPlacement &placement,
                                                 const ::harmos::client::PluginValue &value, client::PluginsCreateOptions options = {}) {
    detail::PluginsCreateRequest request;
    request.set_request_id(options.request_id ? *options.request_id : ::harmos::detail::request_id());
    request.set_name(name);
    *request.mutable_placement() = placement;
    *request.mutable_value() = value;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::PluginsCreateResponse response;
    const auto status = detail::PluginsCreateService::NewStub(channel)->Create(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.create: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::PluginsSetResponse set(const std::string &name, const ::harmos::client::PluginValue &value,
                                           const std::string &expected_revision, client::PluginsSetOptions options = {}) {
    detail::PluginsSetRequest request;
    request.set_name(name);
    *request.mutable_value() = value;
    request.set_expected_revision(expected_revision);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::PluginsSetResponse response;
    const auto status = detail::PluginsSetService::NewStub(channel)->Set(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.set: " + status.error_message());
    }
    return response;
  }
  ::harmos::Receipt delete_(const std::string &name, const std::string &expected_revision, client::PluginsDeleteOptions options = {}) {
    detail::PluginsDeleteRequest request;
    request.set_name(name);
    request.set_expected_revision(expected_revision);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::PluginsDeleteResponse response;
    const auto status = detail::PluginsDeleteService::NewStub(channel)->Delete(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.delete: " + status.error_message());
    }
    if (!response.has_receipt()) {
      throw std::runtime_error("Read plugins.delete: Missing Payload");
    }
    return std::move(*response.mutable_receipt());
  }
  ::harmos::client::PluginEntry restart(const std::string &name, const std::string &expected_revision,
                                        client::PluginsRestartOptions options = {}) {
    detail::PluginsRestartRequest request;
    request.set_name(name);
    request.set_expected_revision(expected_revision);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::PluginsRestartResponse response;
    const auto status = detail::PluginsRestartService::NewStub(channel)->Restart(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.restart: " + status.error_message());
    }
    if (!response.has_entry()) {
      throw std::runtime_error("Read plugins.restart: Missing Payload");
    }
    return std::move(*response.mutable_entry());
  }
  ::harmos::client::PluginsReplaceResponse replace(const std::string &name, const ::harmos::client::PluginPlacement &placement,
                                                   const std::string &expected_revision, client::PluginsReplaceOptions options = {}) {
    detail::PluginsReplaceRequest request;
    request.set_name(name);
    *request.mutable_placement() = placement;
    request.set_expected_revision(expected_revision);
    request.set_request_id(options.request_id ? *options.request_id : ::harmos::detail::request_id());
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::PluginsReplaceResponse response;
    const auto status = detail::PluginsReplaceService::NewStub(channel)->Replace(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.replace: " + status.error_message());
    }
    return response;
  }
};
struct SchemasResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit SchemasResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  std::vector<::harmos::client::SchemaDescription> list(client::SchemasListOptions options = {}) {
    detail::SchemasListRequest request;
    if (options.schema) {
      request.set_schema(*options.schema);
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::SchemasListResponse response;
    const auto status = detail::SchemasListService::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call schemas.list: " + status.error_message());
    }
    return std::vector<::harmos::client::SchemaDescription>(response.schemas().begin(), response.schemas().end());
  }
};
struct StatusResource {
  std::shared_ptr<grpc::Channel> channel;
  StatusDiagnosticsResource diagnostics;
  StatusLogsResource logs;
  StatusTraceEventsResource trace_events;
  explicit StatusResource(std::shared_ptr<grpc::Channel> channel)
      : channel(channel), diagnostics(channel), logs(channel), trace_events(channel) {}
  ::harmos::client::StatusGetResponse get(client::StatusGetOptions options = {}) {
    detail::StatusGetRequest request;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::StatusGetResponse response;
    const auto status = detail::StatusGetService::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call status.get: " + status.error_message());
    }
    return response;
  }
  ::harmos::ClientStream<::harmos::client::StatusWatchResponse> watch(client::StatusWatchOptions options = {}) {
    detail::StatusWatchRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    auto reader = std::shared_ptr<grpc::ClientReader<detail::StatusWatchResponse>>(
        detail::StatusWatchService::NewStub(channel)->Watch(context.get(), request));
    return {context, [reader]() -> std::optional<::harmos::client::StatusWatchResponse> {
              detail::StatusWatchResponse response;
              if (!reader->Read(&response)) {
                const auto status = reader->Finish();
                if (!status.ok()) {
                  throw std::runtime_error("Read status.watch: " + status.error_message());
                }
                return {};
              }
              return response;
            }};
  }
};
struct StreamsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StreamsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  std::vector<::harmos::client::StreamIdentity> list(client::StreamsListOptions options = {}) {
    detail::StreamsListRequest request;
    if (options.schema) {
      request.set_schema(*options.schema);
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::StreamsListResponse response;
    const auto status = detail::StreamsListService::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call streams.list: " + status.error_message());
    }
    return std::vector<::harmos::client::StreamIdentity>(response.streams().begin(), response.streams().end());
  }
};
struct TimeResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit TimeResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::google::protobuf::Timestamp get(client::TimeGetOptions options = {}) {
    detail::TimeGetRequest request;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::TimeGetResponse response;
    const auto status = detail::TimeGetService::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call time.get: " + status.error_message());
    }
    if (!response.has_now()) {
      throw std::runtime_error("Read time.get: Missing Payload");
    }
    return std::move(*response.mutable_now());
  }
  ::harmos::TimeSleepResponse sleep(const ::google::protobuf::Duration &duration, client::TimeSleepOptions options = {}) {
    detail::TimeSleepRequest request;
    *request.mutable_duration() = duration;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::TimeSleepResponse response;
    const auto status = detail::TimeSleepService::NewStub(channel)->Sleep(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call time.sleep: " + status.error_message());
    }
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
  explicit Client(std::shared_ptr<grpc::Channel> channel)
      : access(channel), invocations(channel), operations(channel), plugins(channel), schemas(channel), status(channel), streams(channel),
        time(channel) {}
  static Client connect(std::string address, const ::harmos::StorageTransferSettings &transfer = {},
                        std::optional<int> message_bytes = {}) {
    const auto maximum = ::harmos::transfer_message_bytes(transfer, message_bytes);
    grpc::ChannelArguments arguments;
    arguments.SetMaxSendMessageSize(maximum);
    arguments.SetMaxReceiveMessageSize(maximum);
    if (address.starts_with("http://")) {
      address.erase(0, 7);
    }
    return Client(grpc::CreateCustomChannel(address, grpc::InsecureChannelCredentials(), arguments));
  }
};
} // namespace harmos
