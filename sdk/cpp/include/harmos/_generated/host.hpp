// @generated from protocol descriptors; do not edit.
#pragma once
namespace harmos::plugin {
enum class HostOperation {
  InvocationsGet,
  InvocationsCancel,
  InvocationsWatch,
  OperationsList,
  OperationsGet,
  SettingsList,
  TimeGet,
  TimeSleep,
};
struct HostInvocationsGetOptions {
  std::optional<OperationPolicy> policy;
};
struct HostInvocationsCancelOptions {
  std::optional<OperationPolicy> policy;
};
struct HostInvocationsWatchOptions {};
class HostInvocationsWatch {
public:
  explicit HostInvocationsWatch(std::function<std::optional<::harmos::InstanceState>()> next) : next_(std::move(next)) {}
  std::optional<::harmos::InstanceState> next() {
    return next_();
  }

private:
  std::function<std::optional<::harmos::InstanceState>()> next_;
};
struct HostOperationsListOptions {
  std::optional<OperationPolicy> policy;
};
struct HostOperationsGetOptions {
  std::optional<OperationPolicy> policy;
};
struct HostSettingsListOptions {
  std::optional<::harmos::ReadCut> at;
  std::optional<OperationPolicy> policy;
};
struct HostTimeGetOptions {
  std::optional<OperationPolicy> policy;
};
struct HostTimeSleepOptions {
  std::optional<OperationPolicy> policy;
};
struct HostInvocationsResource {
  Context context;
  ::harmos::InstanceState get(const std::uint64_t &instance, HostInvocationsGetOptions options = {});
  ::harmos::InstanceState cancel(const std::uint64_t &instance, const std::string &reason, HostInvocationsCancelOptions options = {});
  HostInvocationsWatch watch(const std::uint64_t &instance, std::size_t max_item_bytes, HostInvocationsWatchOptions options = {});
};
struct HostOperationsResource {
  Context context;
  std::vector<::harmos::DiscoveredOperation> list(HostOperationsListOptions options = {});
  ::harmos::DiscoveredOperation get(const std::string &id, HostOperationsGetOptions options = {});
};
struct HostSettingsResource {
  Context context;
  ::harmos::plugin::SettingsListResponse list(HostSettingsListOptions options = {});
};
struct HostTimeResource {
  Context context;
  ::google::protobuf::Timestamp get(HostTimeGetOptions options = {});
  ::harmos::plugin::TimeSleepResponse sleep(const ::google::protobuf::Duration &duration, HostTimeSleepOptions options = {});
};
struct HostRoot {
  HostInvocationsResource invocations;
  HostOperationsResource operations;
  HostSettingsResource settings;
  HostTimeResource time;
};
} // namespace harmos::plugin
