// @generated from protocol descriptors; do not edit.
#pragma once
#include "detail/client.hpp"
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
  explicit AccessCertificatesResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  std::string create(client::AccessCertificatesCreateOptions options = {}) {
    detail::packages::harmos::host::access::CertificatesCreateRequest request;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::access::CertificatesCreateResponse response;
    const auto status = detail::packages::harmos::host::access::Certificates::NewStub(channel)->Create(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.certificates.create: " + status.error_message());
    }
    return response.certificate_pem();
  }
};
struct AccessListenerResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit AccessListenerResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  ::harmos::client::ListenerGetResponse get(client::AccessListenerGetOptions options = {}) {
    detail::packages::harmos::host::access::ListenerGetRequest request;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::access::ListenerGetResponse response;
    const auto status = detail::packages::harmos::host::access::Listener::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.listener.get: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::ListenerSetResponse set(const ::harmos::client::ListenerValue &value, const std::string &expected_revision,
                                            client::AccessListenerSetOptions options = {}) {
    detail::packages::harmos::host::access::ListenerSetRequest request;
    *request.mutable_value() = value;
    request.set_expected_revision(expected_revision);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::access::ListenerSetResponse response;
    const auto status = detail::packages::harmos::host::access::Listener::NewStub(channel)->Set(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.listener.set: " + status.error_message());
    }
    return response;
  }
};
struct AccessPairingsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit AccessPairingsResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  ::harmos::client::PairingsCreateResponse create(const std::string &label, client::AccessPairingsCreateOptions options = {}) {
    detail::packages::harmos::host::access::PairingsCreateRequest request;
    request.set_label(label);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::access::PairingsCreateResponse response;
    const auto status = detail::packages::harmos::host::access::Pairings::NewStub(channel)->Create(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.pairings.create: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::PairingsListResponse list(client::AccessPairingsListOptions options = {}) {
    detail::packages::harmos::host::access::PairingsListRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::access::PairingsListResponse response;
    const auto status = detail::packages::harmos::host::access::Pairings::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.pairings.list: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::PairingsDeleteResponse delete_(const std::string &name, client::AccessPairingsDeleteOptions options = {}) {
    detail::packages::harmos::host::access::PairingsDeleteRequest request;
    request.set_name(name);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::access::PairingsDeleteResponse response;
    const auto status = detail::packages::harmos::host::access::Pairings::NewStub(channel)->Delete(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call access.pairings.delete: " + status.error_message());
    }
    return response;
  }
};
struct PluginsSettingsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit PluginsSettingsResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  ::harmos::SettingsListResponse list(client::PluginsSettingsListOptions options = {}) {
    detail::packages::harmos::host::plugins::SettingsListRequest request;
    if (options.name) {
      request.set_name(*options.name);
    }
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::plugins::SettingsListResponse response;
    const auto status = detail::packages::harmos::host::plugins::Settings::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.settings.list: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::SettingsSetResponse set(const std::string &name, const ::harmos::SettingValues &value,
                                            const std::string &expected_revision, client::PluginsSettingsSetOptions options = {}) {
    detail::packages::harmos::host::plugins::SettingsSetRequest request;
    request.set_name(name);
    *request.mutable_value() = value;
    request.set_expected_revision(expected_revision);
    request.set_request_id(options.request_id ? *options.request_id : ::harmos::detail::request_id());
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::plugins::SettingsSetResponse response;
    const auto status = detail::packages::harmos::host::plugins::Settings::NewStub(channel)->Set(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.settings.set: " + status.error_message());
    }
    return response;
  }
};
struct StatusDiagnosticsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StatusDiagnosticsResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  ::harmos::client::DiagnosticsGetResponse get(const std::string &name, const std::uint64_t &offset, const std::uint64_t &max_bytes,
                                               client::StatusDiagnosticsGetOptions options = {}) {
    detail::packages::harmos::host::status::DiagnosticsGetRequest request;
    request.set_name(name);
    request.set_offset(offset);
    request.set_max_bytes(max_bytes);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::status::DiagnosticsGetResponse response;
    const auto status = detail::packages::harmos::host::status::Diagnostics::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call status.diagnostics.get: " + status.error_message());
    }
    return response;
  }
};
struct StatusLogsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit StatusLogsResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  ::harmos::client::LogsListResponse list(client::StatusLogsListOptions options = {}) {
    detail::packages::harmos::host::status::LogsListRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    if (options.page) {
      *request.mutable_page() = *options.page;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::status::LogsListResponse response;
    const auto status = detail::packages::harmos::host::status::Logs::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call status.logs.list: " + status.error_message());
    }
    return response;
  }
  ::harmos::ClientStream<::harmos::client::LogsWatchResponse> watch(const std::uint64_t &max_bytes, const std::uint32_t &max_events,
                                                                    client::StatusLogsWatchOptions options = {}) {
    detail::packages::harmos::host::status::LogsWatchRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    request.set_max_bytes(max_bytes);
    request.set_max_events(max_events);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    auto reader = std::shared_ptr<grpc::ClientReader<detail::packages::harmos::host::status::LogsWatchResponse>>(
        detail::packages::harmos::host::status::Logs::NewStub(channel)->Watch(context.get(), request));
    return {context, [reader]() -> std::optional<::harmos::client::LogsWatchResponse> {
              detail::packages::harmos::host::status::LogsWatchResponse response;
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
  explicit StatusTraceEventsResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  ::harmos::client::TraceEventsListResponse list(client::StatusTraceEventsListOptions options = {}) {
    detail::packages::harmos::host::status::TraceEventsListRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    if (options.page) {
      *request.mutable_page() = *options.page;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::status::TraceEventsListResponse response;
    const auto status = detail::packages::harmos::host::status::TraceEvents::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call status.trace_events.list: " + status.error_message());
    }
    return response;
  }
  ::harmos::ClientStream<::harmos::client::TraceEventsWatchResponse> watch(const std::uint64_t &max_bytes, const std::uint32_t &max_events,
                                                                           client::StatusTraceEventsWatchOptions options = {}) {
    detail::packages::harmos::host::status::TraceEventsWatchRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    request.set_max_bytes(max_bytes);
    request.set_max_events(max_events);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    auto reader = std::shared_ptr<grpc::ClientReader<detail::packages::harmos::host::status::TraceEventsWatchResponse>>(
        detail::packages::harmos::host::status::TraceEvents::NewStub(channel)->Watch(context.get(), request));
    return {context, [reader]() -> std::optional<::harmos::client::TraceEventsWatchResponse> {
              detail::packages::harmos::host::status::TraceEventsWatchResponse response;
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
  explicit AccessResource(std::shared_ptr<grpc::Channel> channel_value)
      : certificates(channel_value), listener(channel_value), pairings(channel_value) {}
};
struct InvocationsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit InvocationsResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  std::string create(const std::string &operation_value, const std::string &fingerprint, const std::string &payload,
                     client::InvocationsCreateOptions options = {}) {
    detail::packages::harmos::host::InvocationsCreateRequest request;
    request.set_operation(operation_value);
    request.set_fingerprint(fingerprint);
    request.set_payload(payload);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::InvocationsCreateResponse response;
    const auto status = detail::packages::harmos::host::Invocations::NewStub(channel)->Create(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call invocations.create: " + status.error_message());
    }
    return response.payload();
  }
  ::harmos::InstanceState get(const std::uint64_t &instance, client::InvocationsGetOptions options = {}) {
    detail::packages::harmos::host::InvocationsGetRequest request;
    request.set_instance(instance);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::InvocationsGetResponse response;
    const auto status = detail::packages::harmos::host::Invocations::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call invocations.get: " + status.error_message());
    }
    if (!response.has_invocation()) {
      throw std::runtime_error("Read invocations.get: Missing Payload");
    }
    return std::move(*response.mutable_invocation());
  }
  ::harmos::InstanceState cancel(const std::uint64_t &instance, const std::string &reason, client::InvocationsCancelOptions options = {}) {
    detail::packages::harmos::host::InvocationsCancelRequest request;
    request.set_instance(instance);
    request.set_reason(reason);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::InvocationsCancelResponse response;
    const auto status = detail::packages::harmos::host::Invocations::NewStub(channel)->Cancel(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call invocations.cancel: " + status.error_message());
    }
    if (!response.has_invocation()) {
      throw std::runtime_error("Read invocations.cancel: Missing Payload");
    }
    return std::move(*response.mutable_invocation());
  }
  ::harmos::ClientStream<::harmos::InstanceState> watch(const std::uint64_t &instance, client::InvocationsWatchOptions options = {}) {
    detail::packages::harmos::host::InvocationsWatchRequest request;
    request.set_instance(instance);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    auto reader = std::shared_ptr<grpc::ClientReader<detail::packages::harmos::host::InvocationsWatchResponse>>(
        detail::packages::harmos::host::Invocations::NewStub(channel)->Watch(context.get(), request));
    return {context, [reader]() -> std::optional<::harmos::InstanceState> {
              detail::packages::harmos::host::InvocationsWatchResponse response;
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
  explicit OperationsResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  std::vector<::harmos::DiscoveredOperation> list(client::OperationsListOptions options = {}) {
    detail::packages::harmos::host::OperationsListRequest request;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::OperationsListResponse response;
    const auto status = detail::packages::harmos::host::Operations::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call operations.list: " + status.error_message());
    }
    return std::vector<::harmos::DiscoveredOperation>(response.operations().begin(), response.operations().end());
  }
  ::harmos::DiscoveredOperation get(const std::string &id, client::OperationsGetOptions options = {}) {
    detail::packages::harmos::host::OperationsGetRequest request;
    request.set_id(id);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::OperationsGetResponse response;
    const auto status = detail::packages::harmos::host::Operations::NewStub(channel)->Get(context.get(), request, &response);
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
  explicit PluginsResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value), settings(channel_value) {}
  ::harmos::client::PluginsGetResponse get(const std::string &name, client::PluginsGetOptions options = {}) {
    detail::packages::harmos::host::PluginsGetRequest request;
    request.set_name(name);
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::PluginsGetResponse response;
    const auto status = detail::packages::harmos::host::Plugins::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.get: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::PluginsListResponse list(client::PluginsListOptions options = {}) {
    detail::packages::harmos::host::PluginsListRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::PluginsListResponse response;
    const auto status = detail::packages::harmos::host::Plugins::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.list: " + status.error_message());
    }
    return response;
  }
  ::harmos::ClientStream<::harmos::client::PluginsWatchResponse> watch(client::PluginsWatchOptions options = {}) {
    detail::packages::harmos::host::PluginsWatchRequest request;
    if (options.name) {
      request.set_name(*options.name);
    }
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    auto reader = std::shared_ptr<grpc::ClientReader<detail::packages::harmos::host::PluginsWatchResponse>>(
        detail::packages::harmos::host::Plugins::NewStub(channel)->Watch(context.get(), request));
    return {context, [reader]() -> std::optional<::harmos::client::PluginsWatchResponse> {
              detail::packages::harmos::host::PluginsWatchResponse response;
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
    detail::packages::harmos::host::PluginsCreateRequest request;
    request.set_request_id(options.request_id ? *options.request_id : ::harmos::detail::request_id());
    request.set_name(name);
    *request.mutable_placement() = placement;
    *request.mutable_value() = value;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::PluginsCreateResponse response;
    const auto status = detail::packages::harmos::host::Plugins::NewStub(channel)->Create(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.create: " + status.error_message());
    }
    return response;
  }
  ::harmos::client::PluginsSetResponse set(const std::string &name, const ::harmos::client::PluginValue &value,
                                           const std::string &expected_revision, client::PluginsSetOptions options = {}) {
    detail::packages::harmos::host::PluginsSetRequest request;
    request.set_name(name);
    *request.mutable_value() = value;
    request.set_expected_revision(expected_revision);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::PluginsSetResponse response;
    const auto status = detail::packages::harmos::host::Plugins::NewStub(channel)->Set(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.set: " + status.error_message());
    }
    return response;
  }
  ::harmos::Receipt delete_(const std::string &name, const std::string &expected_revision, client::PluginsDeleteOptions options = {}) {
    detail::packages::harmos::host::PluginsDeleteRequest request;
    request.set_name(name);
    request.set_expected_revision(expected_revision);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::PluginsDeleteResponse response;
    const auto status = detail::packages::harmos::host::Plugins::NewStub(channel)->Delete(context.get(), request, &response);
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
    detail::packages::harmos::host::PluginsRestartRequest request;
    request.set_name(name);
    request.set_expected_revision(expected_revision);
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::PluginsRestartResponse response;
    const auto status = detail::packages::harmos::host::Plugins::NewStub(channel)->Restart(context.get(), request, &response);
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
    detail::packages::harmos::host::PluginsReplaceRequest request;
    request.set_name(name);
    *request.mutable_placement() = placement;
    request.set_expected_revision(expected_revision);
    request.set_request_id(options.request_id ? *options.request_id : ::harmos::detail::request_id());
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::PluginsReplaceResponse response;
    const auto status = detail::packages::harmos::host::Plugins::NewStub(channel)->Replace(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call plugins.replace: " + status.error_message());
    }
    return response;
  }
};
struct SchemasResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit SchemasResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  std::vector<::harmos::client::SchemaDescription> list(client::SchemasListOptions options = {}) {
    detail::packages::harmos::host::SchemasListRequest request;
    if (options.schema) {
      request.set_schema(*options.schema);
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::SchemasListResponse response;
    const auto status = detail::packages::harmos::host::Schemas::NewStub(channel)->List(context.get(), request, &response);
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
  explicit StatusResource(std::shared_ptr<grpc::Channel> channel_value)
      : channel(channel_value), diagnostics(channel_value), logs(channel_value), trace_events(channel_value) {}
  ::harmos::client::StatusGetResponse get(client::StatusGetOptions options = {}) {
    detail::packages::harmos::host::StatusGetRequest request;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::StatusGetResponse response;
    const auto status = detail::packages::harmos::host::Status::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call status.get: " + status.error_message());
    }
    return response;
  }
  ::harmos::ClientStream<::harmos::client::StatusWatchResponse> watch(client::StatusWatchOptions options = {}) {
    detail::packages::harmos::host::StatusWatchRequest request;
    if (options.at) {
      *request.mutable_at() = *options.at;
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    auto reader = std::shared_ptr<grpc::ClientReader<detail::packages::harmos::host::StatusWatchResponse>>(
        detail::packages::harmos::host::Status::NewStub(channel)->Watch(context.get(), request));
    return {context, [reader]() -> std::optional<::harmos::client::StatusWatchResponse> {
              detail::packages::harmos::host::StatusWatchResponse response;
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
  explicit StreamsResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  std::vector<::harmos::client::StreamIdentity> list(client::StreamsListOptions options = {}) {
    detail::packages::harmos::host::StreamsListRequest request;
    if (options.schema) {
      request.set_schema(*options.schema);
    }
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::StreamsListResponse response;
    const auto status = detail::packages::harmos::host::Streams::NewStub(channel)->List(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call streams.list: " + status.error_message());
    }
    return std::vector<::harmos::client::StreamIdentity>(response.streams().begin(), response.streams().end());
  }
};
struct TimeResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit TimeResource(std::shared_ptr<grpc::Channel> channel_value) : channel(channel_value) {}
  ::google::protobuf::Timestamp get(client::TimeGetOptions options = {}) {
    detail::packages::harmos::host::TimeGetRequest request;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::TimeGetResponse response;
    const auto status = detail::packages::harmos::host::Time::NewStub(channel)->Get(context.get(), request, &response);
    if (!status.ok()) {
      throw std::runtime_error("Call time.get: " + status.error_message());
    }
    if (!response.has_now()) {
      throw std::runtime_error("Read time.get: Missing Payload");
    }
    return std::move(*response.mutable_now());
  }
  ::harmos::TimeSleepResponse sleep(const ::google::protobuf::Duration &duration, client::TimeSleepOptions options = {}) {
    detail::packages::harmos::host::TimeSleepRequest request;
    *request.mutable_duration() = duration;
    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();
    detail::packages::harmos::host::TimeSleepResponse response;
    const auto status = detail::packages::harmos::host::Time::NewStub(channel)->Sleep(context.get(), request, &response);
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
  explicit Client(std::shared_ptr<grpc::Channel> channel_value)
      : access(channel_value), invocations(channel_value), operations(channel_value), plugins(channel_value), schemas(channel_value),
        status(channel_value), streams(channel_value), time(channel_value) {}
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
