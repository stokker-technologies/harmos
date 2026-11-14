/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#pragma once

/* === Imports === */

// Standard Library Imports
#include <any>
#include <cstdint>
#include <functional>
#include <future>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <typeindex>
#include <unordered_map>
#include <utility>
#include <vector>

// Third Party Imports
#include <google/protobuf/message.h>

// Local Imports
#include "harmos/_generated/client.hpp"
#include "harmos/plugin/v1/invocation.pb.h"
#include "harmos/transfer.hpp"

/* === Macros === */

/* === Constants === */

/* === Types === */

namespace harmos::plugin {
enum class HostOperation;
struct HostRoot;
class Context;
template <class Q, class R> class Operation;
template <class R> class Call;
template <class R> class Watching;
template <class R> class Stream;
template <class R> class Observer;

namespace detail {
std::string encode(const google::protobuf::Message &message);
std::string sha256(const std::string &bytes);
google::protobuf::FileDescriptorSet types(const google::protobuf::Descriptor *descriptor);
void merge(google::protobuf::FileDescriptorSet &target, const google::protobuf::FileDescriptorSet &source);
struct OperationDefinition {
  OperationContract contract;
  bool watch = false;
  google::protobuf::FileDescriptorSet types;
  std::function<std::string(Context &, const std::string &)> handler;
};
struct StreamDefinition {
  StreamDeclaration declaration;
  google::protobuf::FileDescriptorSet types;
  std::function<std::string(Context &)> observer;
  std::function<void(Context &, std::function<void(std::string, std::optional<google::protobuf::Timestamp>)>)> producer;
};
struct Invocation;
struct Watch {
  virtual ~Watch() = default;
  virtual std::optional<std::string> next() = 0;
  virtual void cancel() = 0;
};
} // namespace detail

/* === Classes === */

} // namespace harmos::plugin
namespace harmos {
class Failure : public std::runtime_error {
public:
  FailureValue value;
  explicit Failure(FailureValue failure) : std::runtime_error(failure.message()), value(std::move(failure)) {}
};

class TransportError : public std::runtime_error {
public:
  int code;
  TransportError(int status, const std::string &message) : std::runtime_error(message), code(status) {}
};

} // namespace harmos
namespace harmos::plugin {
class Context {
public:
  HostRoot host();
  std::string invocation;
  const std::string &scope() const;
  std::uint32_t attempt = 0;
  std::uint64_t sequence = 0;

  explicit Context(std::shared_ptr<detail::Invocation> state);
  template <class Q, class R> Call<R> call(const Operation<Q, R> &operation, const Q &request, std::optional<OperationPolicy> policy = {});
  template <class Q, class R> Watching<R> watch(const Operation<Q, R> &operation, const Q &request, std::size_t max_item_bytes);
  void sleep(const google::protobuf::Duration &duration);
  google::protobuf::Timestamp now();
  SettingsListResponse settings();
  std::string cancelled();
  template <class T> void set(T value) {
    resources_[typeid(T)] = std::move(value);
  }
  template <class T> T &get() {
    return std::any_cast<T &>(resources_.at(typeid(T)));
  }

private:
  std::string scope_;
  std::shared_ptr<detail::Invocation> state_;
  std::unordered_map<std::type_index, std::any> resources_;
  std::shared_future<std::string> send(::harmos::plugin::v1::ChildRequest request, std::function<void()> *cancel = nullptr);
  std::shared_ptr<detail::Watch> open_watch(::harmos::plugin::v1::ChildRequest request);
  std::optional<std::shared_future<std::string>> local(const detail::OperationDefinition &operation, const std::string &payload,
                                                       std::optional<OperationPolicy> policy, std::function<void()> *cancel);
};

class Plugin {
public:
  Plugin(std::string id, std::string version);
  Plugin &requires_operation(HostOperation operation);
  template <class Q, class R> Plugin &operation(const Operation<Q, R> &operation) {
    if (operation.definition.watch)
      throw std::invalid_argument("Register " + operation.definition.contract.id() + ": Plugin Operations Are Unary");
    operations_.push_back(operation.definition);
    return *this;
  }
  template <class Q, class R> Plugin &requires_operation(const Operation<Q, R> &operation) {
    if (operation.definition.contract.visibility() == VISIBILITY_PRIVATE)
      throw std::invalid_argument("Require " + operation.definition.contract.id() + ": Private Calls Are Local");
    Requirement requirement;
    requirement.set_id(operation.definition.contract.id());
    requirement.set_fingerprint(operation.definition.contract.fingerprint());
    requirements_.emplace_back(std::move(requirement), operation.definition.types);
    return *this;
  }
  template <class R> Plugin &stream(const Stream<R> &stream);
  template <class R> Plugin &observer(const Observer<R> &observer);
  template <class T> Plugin &message() {
    detail::merge(messages_, detail::types(T::descriptor()));
    return *this;
  }
  template <class T> Plugin &section(const T &value) {
    Section section;
    section.set_type(T::descriptor()->full_name());
    section.set_payload(detail::encode(value));
    sections_.emplace_back(std::move(section), detail::types(T::descriptor()));
    return *this;
  }
  Plugin &transfer(const ::harmos::StorageTransferSettings &settings) {
    transfer_ = settings;
    return *this;
  }
  Plugin &setting(SettingDeclaration declaration);
  Plugin &on_stop(std::function<void()> cleanup);
  Declarations declarations() const;
  // An Explicit Address Selects Remote Lifecycle; Empty Uses HARMOS_ADDRESS or Loopback.
  void serve(std::string address = {}) const;

private:
  google::protobuf::FileDescriptorSet messages_;
  ::harmos::StorageTransferSettings transfer_;
  SettingsDeclarations settings_;
  std::string id_;
  std::string version_;
  std::vector<detail::OperationDefinition> operations_;
  std::vector<detail::StreamDefinition> streams_;
  std::vector<std::pair<Requirement, google::protobuf::FileDescriptorSet>> requirements_;
  std::vector<std::pair<Section, google::protobuf::FileDescriptorSet>> sections_;
  std::vector<std::function<void()>> cleanup_;
};

/* === Templates === */

template <class T> T decode(const std::string &bytes) {
  static_assert(std::is_base_of_v<google::protobuf::Message, T>);
  T value;
  if (!value.ParseFromString(bytes))
    throw TransportError(3, "Invalid Protobuf Payload");
  return value;
}

template <class R> class Call {
public:
  explicit Call(std::shared_future<std::string> result, std::function<void()> cancel = {})
      : result_(std::move(result)), cancel_(std::move(cancel)) {}
  R get() const {
    return decode<R>(result_.get());
  }
  void cancel() const {
    if (cancel_)
      cancel_();
  }

private:
  std::shared_future<std::string> result_;
  std::function<void()> cancel_;
};

template <class Q, class R> class Operation {
private:
  friend class Plugin;
  friend class Context;
  detail::OperationDefinition definition;

public:
  explicit Operation(std::string id, Visibility visibility, std::function<R(Context &, Q)> handler = {}) {
    auto request = detail::types(Q::descriptor());
    auto response = detail::types(R::descriptor());
    auto &contract = definition.contract;
    contract.set_id(id);
    contract.set_request(Q::descriptor()->full_name());
    contract.set_response(R::descriptor()->full_name());
    contract.set_visibility(visibility);
    contract.set_fingerprint(detail::sha256(id + std::string(1, '\0') + detail::encode(request) + detail::encode(response)));
    definition.types = request;
    detail::merge(definition.types, response);
    if (handler)
      definition.handler = [handler](Context &ctx, const std::string &bytes) {
        return detail::encode(handler(ctx, decode<Q>(bytes)));
      };
  }
  Operation &visibility(Visibility value) {
    definition.contract.set_visibility(value);
    return *this;
  }
  Operation &watch() {
    if (definition.watch)
      return *this;
    definition.watch = true;
    const auto response = detail::encode(detail::types(R::descriptor()));
    const auto item = detail::sha256(definition.contract.id() + std::string(1, '\0') + response + response);
    definition.contract.set_fingerprint(detail::sha256(std::string(::harmos::detail::stream_fingerprint_prefix, 26) +
                                                       definition.contract.fingerprint() + std::string("\0\1", 2) + item));
    return *this;
  }
  Operation &replay_safe() {
    definition.contract.set_replay_safe(true);
    return *this;
  }
  Operation &policy(OperationPolicy value) {
    *definition.contract.mutable_policy() = std::move(value);
    return *this;
  }
  Operation &description(std::string value) {
    definition.contract.set_description(std::move(value));
    return *this;
  }
};

template <class R> class Watching {
public:
  explicit Watching(std::shared_ptr<detail::Watch> state) : state_(std::move(state)) {}
  Watching(const Watching &) = delete;
  Watching &operator=(const Watching &) = delete;
  Watching(Watching &&) = default;
  ~Watching() {
    if (state_)
      state_->cancel();
  }
  std::optional<R> next() {
    auto value = state_->next();
    return value ? std::optional<R>(decode<R>(*value)) : std::nullopt;
  }

private:
  std::shared_ptr<detail::Watch> state_;
};

template <class Q, class R> Watching<R> Context::watch(const Operation<Q, R> &operation, const Q &request, std::size_t max_item_bytes) {
  if (!operation.definition.watch)
    throw std::invalid_argument("Watch " + operation.definition.contract.id() + ": Server-Streaming Contract Required");
  ::harmos::plugin::v1::ChildRequest child;
  child.set_operation(operation.definition.contract.id());
  child.set_fingerprint(operation.definition.contract.fingerprint());
  child.set_payload(detail::encode(request));
  child.set_watch(true);
  child.set_max_response_bytes(max_item_bytes);
  return Watching<R>(open_watch(std::move(child)));
}

template <class Q, class R>
Call<R> Context::call(const Operation<Q, R> &operation, const Q &request, std::optional<OperationPolicy> policy) {
  if (operation.definition.watch)
    throw std::invalid_argument("Call " + operation.definition.contract.id() + ": Use Watch for a Streaming Contract");
  const auto payload = detail::encode(request);
  std::function<void()> cancel;
  if (auto result = local(operation.definition, payload, policy, &cancel))
    return Call<R>(*result, std::move(cancel));
  ::harmos::plugin::v1::ChildRequest child;
  child.set_operation(operation.definition.contract.id());
  child.set_fingerprint(operation.definition.contract.fingerprint());
  child.set_payload(detail::encode(request));
  if (policy)
    *child.mutable_policy() = std::move(*policy);
  auto result = send(std::move(child), &cancel);
  return Call<R>(std::move(result), std::move(cancel));
}

template <class R> class Publisher {
public:
  using Send = std::function<void(std::string, std::optional<google::protobuf::Timestamp>)>;
  explicit Publisher(Send send) : send_(std::move(send)) {}
  void publish(const R &row, std::optional<google::protobuf::Timestamp> at = {}) const {
    send_(detail::encode(row), std::move(at));
  }

private:
  Send send_;
};

template <class R> class Stream {
private:
  friend class Plugin;
  friend class Observer<R>;
  detail::StreamDefinition definition;

public:
  explicit Stream(std::string id) {
    definition.declaration.set_id(std::move(id));
    definition.declaration.set_row(R::descriptor()->full_name());
    definition.types = detail::types(R::descriptor());
  }
  Stream &publish_on(std::function<void(Context &, Publisher<R>)> producer) {
    definition.producer = [producer](Context &ctx, typename Publisher<R>::Send send) {
      producer(ctx, Publisher<R>(std::move(send)));
    };
    return *this;
  }
  Stream &description(std::string value) {
    definition.declaration.set_description(std::move(value));
    return *this;
  }
};

template <class R> class Observer {
private:
  friend class Plugin;
  detail::StreamDefinition definition;

public:
  Observer(const Stream<R> &stream, std::function<R(Context &)> handler) : definition(stream.definition) {
    definition.observer = [handler](Context &ctx) {
      return detail::encode(handler(ctx));
    };
  }
  Observer &every(const google::protobuf::Duration &interval) {
    *definition.declaration.mutable_interval() = interval;
    return *this;
  }
  Observer &interval_setting(std::string id) {
    definition.declaration.set_interval_setting(std::move(id));
    return *this;
  }
};

template <class R> Plugin &Plugin::stream(const Stream<R> &stream) {
  streams_.push_back(stream.definition);
  return *this;
}
template <class R> Plugin &Plugin::observer(const Observer<R> &observer) {
  for (auto &stream : streams_) {
    if (stream.declaration.id() != observer.definition.declaration.id())
      continue;
    if (stream.observer || stream.producer || observer.definition.producer)
      throw std::invalid_argument("Duplicate Stream Producer");
    const bool same_row = stream.declaration.row() == observer.definition.declaration.row();
    const bool same_types = detail::encode(stream.types) == detail::encode(observer.definition.types);
    if (!same_row || !same_types)
      throw std::invalid_argument("Observer Row Differs from Its Stream");
    stream.observer = observer.definition.observer;
    if (observer.definition.declaration.has_interval()) {
      *stream.declaration.mutable_interval() = observer.definition.declaration.interval();
    }
    stream.declaration.set_interval_setting(observer.definition.declaration.interval_setting());
    return *this;
  }
  throw std::invalid_argument("Observer Requires a Declared Stream");
}

/* === Public Declarations === */

} // namespace harmos::plugin

#include "harmos/_generated/host.hpp"
