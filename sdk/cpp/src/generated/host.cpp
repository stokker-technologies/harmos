// @generated from protocol descriptors; do not edit.
#include "harmos/harmos.hpp"
#include "harmos/plugin/host/v1/invocations.pb.h"
#include "harmos/plugin/host/v1/operations.pb.h"
#include "harmos/plugin/host/v1/settings.pb.h"
#include "harmos/plugin/host/v1/time.pb.h"
using ::harmos::plugin::host::v1::InvocationsCancelRequest;
using ::harmos::plugin::host::v1::InvocationsCancelResponse;
using ::harmos::plugin::host::v1::InvocationsGetRequest;
using ::harmos::plugin::host::v1::InvocationsGetResponse;
using ::harmos::plugin::host::v1::InvocationsWatchRequest;
using ::harmos::plugin::host::v1::InvocationsWatchResponse;
using ::harmos::plugin::host::v1::OperationsGetRequest;
using ::harmos::plugin::host::v1::OperationsGetResponse;
using ::harmos::plugin::host::v1::OperationsListRequest;
using ::harmos::plugin::host::v1::OperationsListResponse;
using ::harmos::plugin::host::v1::SettingsListRequest;
using ::harmos::plugin::host::v1::SettingsListResponse;
using ::harmos::plugin::host::v1::TimeGetRequest;
using ::harmos::plugin::host::v1::TimeGetResponse;
using ::harmos::plugin::host::v1::TimeSleepRequest;
using ::harmos::plugin::host::v1::TimeSleepResponse;
namespace harmos::plugin {
::harmos::InstanceState HostInvocationsResource::get(const std::uint64_t &instance, HostInvocationsGetOptions options) {
  InvocationsGetRequest request;
  request.set_instance(instance);
  auto operation = Operation<InvocationsGetRequest, InvocationsGetResponse>("harmos.plugin.host.v1.Invocations/Get", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  if (!response.has_invocation()) {
    throw std::runtime_error("Read InvocationsGet: Missing Payload");
  }
  return std::move(*response.mutable_invocation());
}
::harmos::InstanceState HostInvocationsResource::cancel(const std::uint64_t &instance, const std::string &reason,
                                                        HostInvocationsCancelOptions options) {
  InvocationsCancelRequest request;
  request.set_instance(instance);
  request.set_reason(reason);
  auto operation =
      Operation<InvocationsCancelRequest, InvocationsCancelResponse>("harmos.plugin.host.v1.Invocations/Cancel", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  if (!response.has_invocation()) {
    throw std::runtime_error("Read InvocationsCancel: Missing Payload");
  }
  return std::move(*response.mutable_invocation());
}
HostInvocationsWatch HostInvocationsResource::watch(const std::uint64_t &instance, std::size_t max_item_bytes,
                                                    HostInvocationsWatchOptions options) {
  InvocationsWatchRequest request;
  request.set_instance(instance);
  auto operation =
      Operation<InvocationsWatchRequest, InvocationsWatchResponse>("harmos.plugin.host.v1.Invocations/Watch", VISIBILITY_PUBLIC);
  (void)options;
  operation.watch();
  auto incoming = std::make_shared<Watching<InvocationsWatchResponse>>(context.watch(operation, request, max_item_bytes));
  return HostInvocationsWatch([incoming]() -> std::optional<::harmos::InstanceState> {
    auto response = incoming->next();
    if (!response) {
      return {};
    }
    if (!response->has_invocation()) {
      throw std::runtime_error("Read InvocationsWatch: Missing Payload");
    }
    return std::move(*response->mutable_invocation());
  });
}
std::vector<::harmos::DiscoveredOperation> HostOperationsResource::list(HostOperationsListOptions options) {
  OperationsListRequest request;
  auto operation = Operation<OperationsListRequest, OperationsListResponse>("harmos.plugin.host.v1.Operations/List", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  return std::vector<::harmos::DiscoveredOperation>(response.operations().begin(), response.operations().end());
}
::harmos::DiscoveredOperation HostOperationsResource::get(const std::string &id, HostOperationsGetOptions options) {
  OperationsGetRequest request;
  request.set_id(id);
  auto operation = Operation<OperationsGetRequest, OperationsGetResponse>("harmos.plugin.host.v1.Operations/Get", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  if (!response.has_operation()) {
    throw std::runtime_error("Read OperationsGet: Missing Payload");
  }
  return std::move(*response.mutable_operation());
}
::harmos::plugin::SettingsListResponse HostSettingsResource::list(HostSettingsListOptions options) {
  SettingsListRequest request;
  if (options.at) {
    *request.mutable_at() = *options.at;
  }
  auto operation = Operation<SettingsListRequest, SettingsListResponse>("harmos.plugin.host.v1.Settings/List", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  return response;
}
::google::protobuf::Timestamp HostTimeResource::get(HostTimeGetOptions options) {
  TimeGetRequest request;
  auto operation = Operation<TimeGetRequest, TimeGetResponse>("harmos.plugin.host.v1.Time/Get", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  if (!response.has_now()) {
    throw std::runtime_error("Read TimeGet: Missing Payload");
  }
  return std::move(*response.mutable_now());
}
::harmos::plugin::TimeSleepResponse HostTimeResource::sleep(const ::google::protobuf::Duration &duration, HostTimeSleepOptions options) {
  TimeSleepRequest request;
  *request.mutable_duration() = duration;
  auto operation = Operation<TimeSleepRequest, TimeSleepResponse>("harmos.plugin.host.v1.Time/Sleep", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  return response;
}
} // namespace harmos::plugin
namespace harmos::plugin {
HostRoot Context::host() {
  return {{*this}, {*this}, {*this}, {*this}};
}
Plugin &Plugin::requires_operation(HostOperation operation) {
  switch (operation) {
  case HostOperation::InvocationsGet:
    return requires_operation(
        Operation<InvocationsGetRequest, InvocationsGetResponse>("harmos.plugin.host.v1.Invocations/Get", VISIBILITY_PUBLIC));
  case HostOperation::InvocationsCancel:
    return requires_operation(
        Operation<InvocationsCancelRequest, InvocationsCancelResponse>("harmos.plugin.host.v1.Invocations/Cancel", VISIBILITY_PUBLIC));
  case HostOperation::InvocationsWatch:
    return requires_operation(
        Operation<InvocationsWatchRequest, InvocationsWatchResponse>("harmos.plugin.host.v1.Invocations/Watch", VISIBILITY_PUBLIC).watch());
  case HostOperation::OperationsList:
    return requires_operation(
        Operation<OperationsListRequest, OperationsListResponse>("harmos.plugin.host.v1.Operations/List", VISIBILITY_PUBLIC));
  case HostOperation::OperationsGet:
    return requires_operation(
        Operation<OperationsGetRequest, OperationsGetResponse>("harmos.plugin.host.v1.Operations/Get", VISIBILITY_PUBLIC));
  case HostOperation::SettingsList:
    return requires_operation(
        Operation<SettingsListRequest, SettingsListResponse>("harmos.plugin.host.v1.Settings/List", VISIBILITY_PUBLIC));
  case HostOperation::TimeGet:
    return requires_operation(Operation<TimeGetRequest, TimeGetResponse>("harmos.plugin.host.v1.Time/Get", VISIBILITY_PUBLIC));
  case HostOperation::TimeSleep:
    return requires_operation(Operation<TimeSleepRequest, TimeSleepResponse>("harmos.plugin.host.v1.Time/Sleep", VISIBILITY_PUBLIC));
  default:
    throw std::invalid_argument("Declare Host Requirement: Unknown Operation");
  }
}
} // namespace harmos::plugin
