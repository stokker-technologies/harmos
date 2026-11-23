/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
#include "harmos/host/v1/time.pb.h"
#include "harmos/plugin/v1/subscriptions.pb.h"
#include "invocation.hpp"
#include <google/protobuf/util/json_util.h>
#include <google/protobuf/util/time_util.h>

using google::protobuf::Duration;
using google::protobuf::Timestamp;
using std::int32_t;
using std::int64_t;
using std::invalid_argument;
using std::lock_guard;
using std::optional;
using std::promise;
using std::shared_future;
using std::shared_ptr;
using std::string;
using std::uint32_t;
using std::uint64_t;
using std::unique_lock;
using std::vector;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

namespace harmos::plugin {
namespace {
class ChildWatch final : public detail::Watch {
public:
  shared_ptr<detail::Invocation> state;
  shared_ptr<detail::WatchState> watch;
  uint64_t call;
  std::atomic<bool> closed = false;
  optional<string> next() override {
    if (closed)
      return {};
    bool active;
    {
      lock_guard lock(state->mutex);
      if (watch->demanded)
        throw std::logic_error("Watch: Concurrent Next Is Not Allowed");
      active = !watch->terminal && !state->closed;
      watch->demanded = active;
    }
    if (active) {
      plugin::v1::InstanceInvokeResponse demand;
      demand.mutable_child_demand()->set_call(call);
      state->emit(std::move(demand));
      state->start_reading();
    }
    unique_lock lock(state->mutex);
    state->changed.wait(lock, [&] {
      return watch->item || watch->terminal || state->closed || closed;
    });
    if (closed)
      return {};
    if (watch->item) {
      auto item = std::move(*watch->item);
      watch->item.reset();
      watch->demanded = false;
      return item.item();
    }
    closed = true;
    state->watches.erase(call);
    if (watch->terminal) {
      if (watch->terminal->has_failure())
        throw OperationError(watch->terminal->failure());
      return {};
    }
    throw TransportError(14, "Watch: Invocation Closed");
  }
  void cancel() override {
    {
      lock_guard lock(state->mutex);
      if (closed)
        return;
      closed = true;
      state->watches.erase(call);
      state->retired.insert(call);
      state->changed.notify_all();
    }
    plugin::v1::InstanceInvokeResponse frame;
    frame.mutable_child_cancel()->set_call(call);
    try {
      state->emit(std::move(frame));
    } catch (const TransportError &) {
    }
  }
};
} // namespace

shared_ptr<detail::Watch> Context::open_watch(const detail::OperationDefinition &operation, const string &payload,
                                              std::size_t max_item_bytes) {
  plugin::v1::ChildRequest request;
  request.set_operation(operation.contract.id());
  request.set_fingerprint(operation.contract.fingerprint());
  request.set_payload(payload);
  request.set_watch(true);
  request.set_max_response_bytes(max_item_bytes);
  auto result = std::make_shared<ChildWatch>();
  auto root = state_->transport ? state_->transport : state_;
  result->state = root;
  result->watch = std::make_shared<detail::WatchState>();
  result->watch->max_item_bytes = request.max_response_bytes();
  result->watch->owner = state_;
  {
    lock_guard lock(root->mutex);
    auto found = state_->requirements.find(request.operation());
    if (root->closed || (root->window_scoped && (!root->window_active || !root->accepting_children)) || state_->cancelled)
      throw TransportError(3, "Watch " + request.operation() + ": Child Scope Is Closed");
    if (!root->emit || !request.max_response_bytes() || found == state_->requirements.end() || found->second != request.fingerprint())
      throw invalid_argument("Watch " + request.operation() + ": Declared Requirement and Positive Item Limit Required");
    if (root->next == UINT64_MAX)
      throw TransportError(8, "Watch " + request.operation() + ": Child Identity Exhausted");
    result->call = ++root->next;
    request.set_call(result->call);
    root->watches.emplace(result->call, result->watch);
  }
  plugin::v1::InstanceInvokeResponse frame;
  *frame.mutable_child() = std::move(request);
  root->emit(std::move(frame));
  return result;
}

detail::WatchHandle Context::open_subscription(const string &stream, const google::protobuf::Descriptor *row, std::size_t max_item_bytes) {
  plugin::v1::SubscriptionsSubscribeRequest request;
  request.set_stream(stream);
  request.set_row(row->full_name());
  request.set_types(detail::encode(detail::types(row)));
  Operation<plugin::v1::SubscriptionsSubscribeRequest, plugin::v1::SubscriptionsSubscribeResponse> operation(
      "harmos.plugin.v1.Subscriptions/Subscribe", VISIBILITY_PUBLIC);
  operation.watch();
  detail::WatchHandle incoming(open_watch(operation.definition, detail::encode(request), max_item_bytes));
  auto payload = incoming.next();
  if (!payload)
    throw invalid_argument("Subscribe to Stream: Host Must Acknowledge the Admitted Demand");
  const auto opened = decode<plugin::v1::SubscriptionsSubscribeResponse>(*payload);
  if (!opened.has_opened() || !opened.opened())
    throw invalid_argument("Subscribe to Stream: Host Must Acknowledge the Admitted Demand");
  return incoming;
}

std::variant<detail::SubscriptionRow, Gap> detail::subscription_item(const string &payload) {
  const auto response = decode<plugin::v1::SubscriptionsSubscribeResponse>(payload);
  if (response.has_missed())
    return Gap{response.missed()};
  if (!response.has_row() || !response.row().has_source() || !response.row().has_received())
    throw invalid_argument("Read Subscription: Supply a Row With Both Timestamps or a Gap");
  const auto &row = response.row();
  return detail::SubscriptionRow{row.payload(), row.source(), row.received()};
}

Context::Context(shared_ptr<detail::Invocation> state) : state_(std::move(state)) {
  invocation = state_->request.invocation();
  capturing_executions.assign(state_->request.capturing_executions().begin(), state_->request.capturing_executions().end());
  scope_ = state_->request.scope();
  attempt = state_->request.attempt();
}

const string &Context::scope() const {
  if (scope_.empty()) {
    ::harmos::model::v1::Failure failure;
    failure.set_classification(::harmos::model::v1::CLASSIFICATION_REFUSED);
    failure.set_code("scope_required");
    failure.set_message("Invocation Scope Is Required: the caller must bind a scope");
    throw OperationError(std::move(failure));
  }
  return scope_;
}

namespace {
shared_ptr<detail::Invocation> transport(const shared_ptr<detail::Invocation> &state) {
  return state->transport ? state->transport : state;
}
optional<string> cancellation(const shared_ptr<detail::Invocation> &state) {
  if (state->cancelled)
    return state->cancelled;
  if (state->transport)
    return state->transport->cancelled;
  return {};
}
::harmos::model::v1::Failure interrupted(const shared_ptr<detail::Invocation> &state, ::harmos::model::v1::Classification kind,
                                         const string &reason) {
  ::harmos::model::v1::Failure failure;
  failure.set_classification(kind);
  failure.set_code("interrupted");
  failure.set_message("Invoke " + state->request.operation() + ": " + reason);
  failure.set_operation(state->request.operation());
  failure.set_plugin(state->plugin);
  failure.set_action("Invoke Again If Safe");
  return failure;
}
void cancel_owned(const shared_ptr<detail::Invocation> &state, const string &reason, ::harmos::model::v1::Classification kind) {
  auto root = transport(state);
  vector<uint64_t> cancelled;
  {
    lock_guard lock(root->mutex);
    state->cancelled = reason;
    for (auto id : state->owned) {
      auto found = root->pending.find(id);
      if (found == root->pending.end())
        continue;
      found->second.set_exception(std::make_exception_ptr(OperationError(interrupted(state, kind, reason))));
      root->pending.erase(found);

      root->retired.insert(id);
      cancelled.push_back(id);
    }
    for (auto item = root->watches.begin(); item != root->watches.end();) {
      if (item->second->owner.lock() != state) {
        ++item;
        continue;
      }
      plugin::v1::ChildResponse terminal;
      terminal.set_call(item->first);
      *terminal.mutable_failure() = interrupted(state, kind, reason);
      item->second->terminal = std::move(terminal);
      root->retired.insert(item->first);
      cancelled.push_back(item->first);
      item = root->watches.erase(item);
    }
    root->changed.notify_all();
  }
  for (auto id : cancelled) {
    plugin::v1::InstanceInvokeResponse frame;
    frame.mutable_child_cancel()->set_call(id);
    try {
      root->emit(std::move(frame));
    } catch (const TransportError &) {
    }
  }
}
} // namespace

shared_future<string> Context::send(const detail::OperationDefinition &operation, const string &payload,
                                    std::optional<OperationPolicy> policy, std::function<void()> *cancel, optional<string> json) {
  plugin::v1::ChildRequest request;
  request.set_operation(operation.contract.id());
  request.set_fingerprint(operation.contract.fingerprint());
  request.set_payload(payload);
  if (json)
    request.set_json(std::move(*json));
  if (policy)
    *request.mutable_policy() = std::move(*policy);
  auto root = transport(state_);
  shared_future<string> result;
  {
    lock_guard lock(root->mutex);
    if (!root->emit || (root->window_scoped && (!root->window_active || !root->accepting_children)))
      throw TransportError(3, "Invoke " + request.operation() + ": Child Scope Is Closed");
    if (root->closed)
      throw TransportError(14, "Invoke: Invocation Closed");
    if (auto reason = cancellation(state_))
      throw OperationError(interrupted(state_, ::harmos::model::v1::CLASSIFICATION_CANCELLED, *reason));
    if (root->next == UINT64_MAX)
      throw TransportError(8, "Invoke " + request.operation() + ": Child Identity Exhausted");
    request.set_call(++root->next);
    auto &pending = root->pending[request.call()];
    result = pending.get_future().share();
    state_->owned.insert(request.call());
    if (cancel)
      *cancel = [root, owner = state_, id = request.call()] {
        {
          lock_guard child_lock(root->mutex);
          auto found = root->pending.find(id);
          if (found == root->pending.end())
            return;
          found->second.set_exception(std::make_exception_ptr(
              OperationError(interrupted(owner, ::harmos::model::v1::CLASSIFICATION_CANCELLED, "Child Call Cancelled"))));
          root->pending.erase(found);

          root->retired.insert(id);
          root->changed.notify_all();
        }
        plugin::v1::InstanceInvokeResponse frame;
        frame.mutable_child_cancel()->set_call(id);
        root->emit(std::move(frame));
      };
  }
  plugin::v1::InstanceInvokeResponse frame;
  *frame.mutable_child() = std::move(request);
  root->emit(std::move(frame));
  root->start_reading();
  return result;
}

google::protobuf::Struct Context::operation(const string &operation, const google::protobuf::Struct &payload,
                                            optional<OperationPolicy> policy) {
  string text;
  auto status = google::protobuf::util::MessageToJsonString(payload, &text);
  if (!status.ok())
    throw invalid_argument("Call " + operation + ": Encode a JSON Object: " + status.ToString());
  detail::OperationDefinition definition;
  definition.contract.set_id(operation);
  auto reply = send(definition, {}, std::move(policy), nullptr, std::move(text)).get();
  google::protobuf::Struct result;
  status = google::protobuf::util::JsonStringToMessage(reply, &result);
  if (!status.ok())
    throw TransportError(3, "Call " + operation + ": Host Must Return a JSON Object: " + status.ToString());
  return result;
}

bool Context::wind_down_requested() const {
  auto root = state_->transport ? state_->transport : state_;
  return root->wind_down_requested.load();
}

string Context::cancelled() {
  auto root = transport(state_);
  root->start_reading();
  unique_lock lock(root->mutex);
  root->changed.wait(lock, [&] {
    return cancellation(state_).has_value() || root->closed;
  });
  return cancellation(state_).value_or("Invocation Closed");
}

void Context::sleep(const Duration &duration) {
  host().time.sleep(duration);
}

Timestamp Context::now() {
  return host().time.get();
}

HostPluginsStateResource Context::state() {
  return host().plugins.state;
}

SettingsListResponse Context::settings() {
  return host().plugins.settings.list();
}
} // namespace harmos::plugin

namespace harmos::plugin::detail {
void cancel_children(const shared_ptr<Invocation> &state, const string &reason) {
  cancel_owned(state, reason, ::harmos::model::v1::CLASSIFICATION_CANCELLED);
  auto root = transport(state);
  vector<uint64_t> cancelled;
  {
    lock_guard lock(root->mutex);
    for (auto &[id, promise] : root->pending) {
      promise.set_exception(
          std::make_exception_ptr(OperationError(interrupted(state, ::harmos::model::v1::CLASSIFICATION_CANCELLED, reason))));
      root->retired.insert(id);
      cancelled.push_back(id);
    }
    root->pending.clear();

    for (auto &[id, watch] : root->watches) {
      plugin::v1::ChildResponse terminal;
      terminal.set_call(id);
      *terminal.mutable_failure() = interrupted(state, ::harmos::model::v1::CLASSIFICATION_CANCELLED, reason);
      watch->terminal = std::move(terminal);
      root->retired.insert(id);
      cancelled.push_back(id);
    }
    root->watches.clear();
    root->changed.notify_all();
  }
  for (auto id : cancelled) {
    plugin::v1::InstanceInvokeResponse frame;
    frame.mutable_child_cancel()->set_call(id);
    try {
      root->emit(std::move(frame));
    } catch (const TransportError &) {
    }
  }
}
} // namespace harmos::plugin::detail

/* === Private Functions === */

/* === Entry Point === */

namespace harmos {
google::protobuf::Struct json(const std::string &text) {
  google::protobuf::Struct value;
  auto status = google::protobuf::util::JsonStringToMessage(text, &value);
  if (!status.ok())
    throw std::invalid_argument("Parse Operation Payload: Supply a JSON Object: " + status.ToString());
  return value;
}
} // namespace harmos
