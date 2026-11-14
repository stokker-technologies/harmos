// @generated from protocol descriptors; do not edit.
#include "harmos/harmos.hpp"
#include "harmos/host/v1/invocations.pb.h"
#include "harmos/host/v1/operations.pb.h"
#include "harmos/host/v1/plugins/settings.pb.h"
#include "harmos/host/v1/time.pb.h"
using ::google::protobuf::Duration;
using ::google::protobuf::Timestamp;
using ::harmos::DiscoveredOperation;
using ::harmos::InstanceState;
using ::harmos::host::v1::InvocationsCancelRequest;
using ::harmos::host::v1::InvocationsCancelResponse;
using ::harmos::host::v1::InvocationsGetRequest;
using ::harmos::host::v1::InvocationsGetResponse;
using ::harmos::host::v1::InvocationsWatchRequest;
using ::harmos::host::v1::InvocationsWatchResponse;
using ::harmos::host::v1::OperationsGetRequest;
using ::harmos::host::v1::OperationsGetResponse;
using ::harmos::host::v1::OperationsListRequest;
using ::harmos::host::v1::OperationsListResponse;
using ::harmos::host::v1::TimeGetRequest;
using ::harmos::host::v1::TimeGetResponse;
using ::harmos::host::v1::TimeSleepRequest;
using ::harmos::host::v1::TimeSleepResponse;
using ::harmos::host::v1::plugins::SettingsListRequest;
using ::harmos::host::v1::plugins::SettingsListResponse;
using std::invalid_argument;
using std::optional;
using std::runtime_error;
using std::size_t;
using std::string;
using std::uint64_t;
using std::vector;
namespace harmos::plugin {
Timestamp HostTimeResource::get(HostTimeGetOptions options) {
  TimeGetRequest request;
  auto operation = Operation<TimeGetRequest, TimeGetResponse>("harmos.host.v1.Time/Get", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  if (!response.has_now()) {
    throw runtime_error("Read TimeGet: Missing Payload");
  }
  return std::move(*response.mutable_now());
}
TimeSleepResponse HostTimeResource::sleep(const Duration &duration, HostTimeSleepOptions options) {
  TimeSleepRequest request;
  *request.mutable_duration() = duration;
  auto operation = Operation<TimeSleepRequest, TimeSleepResponse>("harmos.host.v1.Time/Sleep", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  return response;
}
SettingsListResponse HostPluginsSettingsResource::list(HostPluginsSettingsListOptions options) {
  SettingsListRequest request;
  if (options.name) {
    request.set_name(*options.name);
  }
  if (options.at) {
    *request.mutable_at() = *options.at;
  }
  auto operation = Operation<SettingsListRequest, SettingsListResponse>("harmos.host.v1.plugins.Settings/List", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  return response;
}
vector<DiscoveredOperation> HostOperationsResource::list(HostOperationsListOptions options) {
  OperationsListRequest request;
  auto operation = Operation<OperationsListRequest, OperationsListResponse>("harmos.host.v1.Operations/List", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  return vector<DiscoveredOperation>(response.operations().begin(), response.operations().end());
}
DiscoveredOperation HostOperationsResource::get(const string &id, HostOperationsGetOptions options) {
  OperationsGetRequest request;
  request.set_id(id);
  auto operation = Operation<OperationsGetRequest, OperationsGetResponse>("harmos.host.v1.Operations/Get", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  if (!response.has_operation()) {
    throw runtime_error("Read OperationsGet: Missing Payload");
  }
  return std::move(*response.mutable_operation());
}
InstanceState HostInvocationsResource::get(const uint64_t &instance, HostInvocationsGetOptions options) {
  InvocationsGetRequest request;
  request.set_instance(instance);
  auto operation = Operation<InvocationsGetRequest, InvocationsGetResponse>("harmos.host.v1.Invocations/Get", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  if (!response.has_invocation()) {
    throw runtime_error("Read InvocationsGet: Missing Payload");
  }
  return std::move(*response.mutable_invocation());
}
InstanceState HostInvocationsResource::cancel(const uint64_t &instance, const string &reason, HostInvocationsCancelOptions options) {
  InvocationsCancelRequest request;
  request.set_instance(instance);
  request.set_reason(reason);
  auto operation = Operation<InvocationsCancelRequest, InvocationsCancelResponse>("harmos.host.v1.Invocations/Cancel", VISIBILITY_PUBLIC);
  auto response = context.call(operation, request, options.policy).get();
  if (!response.has_invocation()) {
    throw runtime_error("Read InvocationsCancel: Missing Payload");
  }
  return std::move(*response.mutable_invocation());
}
HostInvocationsWatch HostInvocationsResource::watch(const uint64_t &instance, size_t max_item_bytes, HostInvocationsWatchOptions options) {
  InvocationsWatchRequest request;
  request.set_instance(instance);
  auto operation = Operation<InvocationsWatchRequest, InvocationsWatchResponse>("harmos.host.v1.Invocations/Watch", VISIBILITY_PUBLIC);
  (void)options;
  operation.watch();
  auto incoming = std::make_shared<Watching<InvocationsWatchResponse>>(context.watch(operation, request, max_item_bytes));
  return HostInvocationsWatch([incoming]() -> optional<InstanceState> {
    auto response = incoming->next();
    if (!response) {
      return {};
    }
    if (!response->has_invocation()) {
      throw runtime_error("Read InvocationsWatch: Missing Payload");
    }
    return std::move(*response->mutable_invocation());
  });
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
        Operation<InvocationsGetRequest, InvocationsGetResponse>("harmos.host.v1.Invocations/Get", VISIBILITY_PUBLIC));
  case HostOperation::InvocationsCancel:
    return requires_operation(
        Operation<InvocationsCancelRequest, InvocationsCancelResponse>("harmos.host.v1.Invocations/Cancel", VISIBILITY_PUBLIC));
  case HostOperation::InvocationsWatch:
    return requires_operation(
        Operation<InvocationsWatchRequest, InvocationsWatchResponse>("harmos.host.v1.Invocations/Watch", VISIBILITY_PUBLIC).watch());
  case HostOperation::OperationsList:
    return requires_operation(
        Operation<OperationsListRequest, OperationsListResponse>("harmos.host.v1.Operations/List", VISIBILITY_PUBLIC));
  case HostOperation::OperationsGet:
    return requires_operation(Operation<OperationsGetRequest, OperationsGetResponse>("harmos.host.v1.Operations/Get", VISIBILITY_PUBLIC));
  case HostOperation::PluginsSettingsList:
    return requires_operation(
        Operation<SettingsListRequest, SettingsListResponse>("harmos.host.v1.plugins.Settings/List", VISIBILITY_PUBLIC));
  case HostOperation::TimeGet:
    return requires_operation(Operation<TimeGetRequest, TimeGetResponse>("harmos.host.v1.Time/Get", VISIBILITY_PUBLIC));
  case HostOperation::TimeSleep:
    return requires_operation(Operation<TimeSleepRequest, TimeSleepResponse>("harmos.host.v1.Time/Sleep", VISIBILITY_PUBLIC));
  default:
    throw invalid_argument("Declare Host Requirement: Unknown Operation");
  }
}
} // namespace harmos::plugin
