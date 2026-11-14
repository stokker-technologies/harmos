// @generated from protocol descriptors; do not edit.
#pragma once
namespace harmos::plugin {
enum class HostOperation {
  InvocationsGet,
  InvocationsCancel,
  InvocationsWatch,
  OperationsList,
  OperationsGet,
  PluginsSettingsList,
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
struct HostPluginsSettingsListOptions {
  std::optional<std::string> name;
  std::optional<::harmos::ReadCut> at;
  std::optional<OperationPolicy> policy;
};
struct HostTimeGetOptions {
  std::optional<OperationPolicy> policy;
};
struct HostTimeSleepOptions {
  std::optional<OperationPolicy> policy;
};
struct HostTimeResource {
  Context context;
  HostTimeResource(Context value) : context(value) {}
  ::google::protobuf::Timestamp get(HostTimeGetOptions options = {});
  ::harmos::TimeSleepResponse sleep(const ::google::protobuf::Duration &duration, HostTimeSleepOptions options = {});
};
struct HostPluginsSettingsResource {
  Context context;
  HostPluginsSettingsResource(Context value) : context(value) {}
  ::harmos::SettingsListResponse list(HostPluginsSettingsListOptions options = {});
};
struct HostPluginsResource {
  Context context;
  HostPluginsSettingsResource settings;
  HostPluginsResource(Context value) : context(value), settings(value) {}
};
struct HostOperationsResource {
  Context context;
  HostOperationsResource(Context value) : context(value) {}
  std::vector<::harmos::DiscoveredOperation> list(HostOperationsListOptions options = {});
  ::harmos::DiscoveredOperation get(const std::string &id, HostOperationsGetOptions options = {});
};
struct HostInvocationsResource {
  Context context;
  HostInvocationsResource(Context value) : context(value) {}
  ::harmos::InstanceState get(const std::uint64_t &instance, HostInvocationsGetOptions options = {});
  ::harmos::InstanceState cancel(const std::uint64_t &instance, const std::string &reason, HostInvocationsCancelOptions options = {});
  HostInvocationsWatch watch(const std::uint64_t &instance, std::size_t max_item_bytes, HostInvocationsWatchOptions options = {});
};
struct HostRoot {
  HostInvocationsResource invocations;
  HostOperationsResource operations;
  HostPluginsResource plugins;
  HostTimeResource time;
};
} // namespace harmos::plugin
