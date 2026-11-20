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
  PluginsStateGet,
  PluginsStatePut,
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
  explicit HostInvocationsWatch(std::function<std::optional<::harmos::InstanceState>()> next_value) : next_(std::move(next_value)) {}
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
struct HostPluginsStateGetOptions {
  std::optional<OperationPolicy> policy;
};
struct HostPluginsStatePutOptions {
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
  HostTimeResource(Context context_value) : context(context_value) {}
  ::google::protobuf::Timestamp get(HostTimeGetOptions options = {});
  ::harmos::plugin::TimeSleepResponse sleep(const ::google::protobuf::Duration &duration, HostTimeSleepOptions options = {});
};
struct HostPluginsStateResource {
  Context context;
  HostPluginsStateResource(Context context_value) : context(context_value) {}
  ::harmos::plugin::StateGetResponse get(HostPluginsStateGetOptions options = {});
  std::uint64_t put(const std::uint64_t &expected_revision, const std::string &value, HostPluginsStatePutOptions options = {});
};
struct HostPluginsSettingsResource {
  Context context;
  HostPluginsSettingsResource(Context context_value) : context(context_value) {}
  ::harmos::plugin::SettingsListResponse list(HostPluginsSettingsListOptions options = {});
};
struct HostPluginsResource {
  Context context;
  HostPluginsSettingsResource settings;
  HostPluginsStateResource state;
  HostPluginsResource(Context context_value) : context(context_value), settings(context_value), state(context_value) {}
};
struct HostOperationsResource {
  Context context;
  HostOperationsResource(Context context_value) : context(context_value) {}
  std::vector<::harmos::DiscoveredOperation> list(HostOperationsListOptions options = {});
  ::harmos::DiscoveredOperation get(const std::string &id, HostOperationsGetOptions options = {});
};
struct HostInvocationsResource {
  Context context;
  HostInvocationsResource(Context context_value) : context(context_value) {}
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
