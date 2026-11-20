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
#include <google/protobuf/duration.pb.h>
#include <google/protobuf/message.h>
#include <google/protobuf/timestamp.pb.h>

// Local Imports
#include "harmos/client_transport.hpp"
#include "harmos/messages.hpp"
#include "harmos/transfer.hpp"

/* === Macros === */

/* === Constants === */

/* === Types === */

namespace harmos::plugin {
enum class HostOperation;
struct HostRoot;
class Context;
class Registrar;
template <class Q, class R> class Operation;
template <class R> class Call;
template <class R> class Watching;
template <class R> class Stream;
template <class R> class Observer;

// One Host Grant of an Interval Stream: Sample i Is at start + i * period with Sequence sequence + i.
struct Window {
  google::protobuf::Timestamp start;
  google::protobuf::Duration period;
  std::uint64_t count = 0;
  std::uint64_t sequence = 0;
};

namespace detail {
std::string encode(const google::protobuf::Message &message);
std::string sha256(const std::string &bytes);
google::protobuf::FileDescriptorSet types(const google::protobuf::Descriptor *descriptor);
void merge(google::protobuf::FileDescriptorSet &target, const google::protobuf::FileDescriptorSet &source);
// Interval Rows Hold One Value per Field; Window Columns Repeat Every Row Field.
void validate_row(const google::protobuf::Descriptor *row);
void validate_columns(const google::protobuf::Descriptor *row, const google::protobuf::Descriptor *columns);
struct OperationDefinition {
  OperationContract contract;
  bool watch = false;
  google::protobuf::FileDescriptorSet types;
  std::function<std::string(Context &, const std::string &)> handler;
};
struct StreamDefinition {
  StreamDeclaration declaration;
  google::protobuf::FileDescriptorSet types;
  const google::protobuf::Descriptor *row = nullptr;
  // A One-Sample Observer Answers Each Sample of a Window with One Row.
  std::function<std::unique_ptr<google::protobuf::Message>(Context &)> sample;
  // A Window Observer Answers a Whole Window with One Columns Message.
  std::function<std::unique_ptr<google::protobuf::Message>(Context &, const Window &)> window;
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
  model::Failure value;
  explicit Failure(model::Failure failure) : std::runtime_error(failure.message()), value(std::move(failure)) {}
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
  std::shared_future<std::string> send(const detail::OperationDefinition &operation, const std::string &payload,
                                       std::optional<OperationPolicy> policy, std::function<void()> *cancel = nullptr);
  std::shared_ptr<detail::Watch> open_watch(const detail::OperationDefinition &operation, const std::string &payload,
                                            std::size_t max_item_bytes);
  std::optional<std::shared_future<std::string>> local(const detail::OperationDefinition &operation, const std::string &payload,
                                                       std::optional<OperationPolicy> policy, std::function<void()> *cancel);
};

template <class T> struct Dependency {
  T operation;
};
template <class T> struct Contribution {
  T value;
};
template <class T> struct Descriptor {};

// Owns the Serving Transport and Its Event Engine; Destruction Drains Accepted Calls.
class Listener {
public:
  Listener(const Listener &) = delete;
  Listener &operator=(const Listener &) = delete;
  Listener(Listener &&) noexcept;
  Listener &operator=(Listener &&) noexcept;
  ~Listener();
  const std::string &address() const;
  // Await Transport Shutdown Without Requesting It.
  void wait();
  void close();

private:
  friend class Plugin;
  struct State;
  explicit Listener(std::unique_ptr<State> state);
  std::unique_ptr<State> state_;
};

class Lane {
public:
  static Lane serial(std::string name) {
    return Lane(std::move(name), 1, true);
  }
  static Lane concurrent(std::string name, std::uint32_t width) {
    return Lane(std::move(name), width, false);
  }
  static Lane unbounded(std::string name) {
    return Lane(std::move(name), std::nullopt, false);
  }
  const ::harmos::LaneDeclaration &declaration() const {
    return declaration_;
  }

private:
  Lane(std::string name, std::optional<std::uint32_t> width, bool serial) {
    if (name.empty() || name == "Open" || name == "harmos.resident") {
      throw std::invalid_argument("Declare Lane `" + name + "`: Choose a Nonempty, Nonreserved Name");
    }
    for (unsigned char character : name) {
      if (character < 32 || character == 127) {
        throw std::invalid_argument("Declare Lane `" + name + "`: Remove Control Characters");
      }
    }
    if (width && *width == 0) {
      throw std::invalid_argument("Declare Lane `" + name + "`: Choose a Positive Concurrent Width");
    }
    declaration_.set_name(std::move(name));
    if (serial) {
      declaration_.set_serial(true);
    } else if (width) {
      declaration_.set_concurrent(*width);
    } else {
      declaration_.set_unbounded(true);
    }
  }
  ::harmos::LaneDeclaration declaration_;
};

class Plugin {
public:
  Plugin(std::string id, std::string version);
  Plugin &add(std::function<void(Registrar &)> catalog);
  Plugin &lane(const Lane &lane);
  Plugin &transfer(const ::harmos::StorageTransferSettings &settings) {
    transfer_ = settings;
    return *this;
  }
  Plugin &on_stop(std::function<void()> cleanup);
  Declarations declarations() const;
  // Binds the Supplied Address; Port Zero Selects a Free Port. Stop Keeps Serving.
  Listener start(std::string address) const;
  // An Explicit Address Selects Remote Lifecycle; Empty Uses HARMOS_ADDRESS or Loopback.
  void run(std::string address = {}) const;

private:
  friend class Registrar;
  Listener start(std::string address, bool remote) const;
  google::protobuf::FileDescriptorSet messages_;
  ::harmos::StorageTransferSettings transfer_;
  SettingsDeclarations settings_;
  std::string id_;
  std::string version_;
  std::vector<detail::OperationDefinition> operations_;
  std::vector<::harmos::LaneDeclaration> lanes_;
  std::vector<detail::StreamDefinition> streams_;
  std::vector<std::pair<Requirement, google::protobuf::FileDescriptorSet>> requirements_;
  std::vector<std::pair<Section, google::protobuf::FileDescriptorSet>> sections_;
  std::vector<std::function<void()>> cleanup_;
};

// Declaration-Only Accumulation; No Runtime Facilities Are Available.
class Registrar {
public:
  Registrar &add(std::function<void(Registrar &)> catalog) {
    catalog(*this);
    return *this;
  }
  Registrar &add(HostOperation operation);
  template <class Q, class R> Registrar &add(const Operation<Q, R> &operation) {
    if (operation.definition.watch)
      throw std::invalid_argument("Register " + operation.definition.contract.id() + ": Plugin Operations Are Unary");
    plugin_.operations_.push_back(operation.definition);
    return *this;
  }
  template <class Q, class R> Registrar &add(const Dependency<Operation<Q, R>> &dependency) {
    const auto &operation = dependency.operation;
    if (operation.definition.contract.visibility() == VISIBILITY_PRIVATE)
      throw std::invalid_argument("Require " + operation.definition.contract.id() + ": Private Calls Are Local");
    Requirement requirement;
    requirement.set_id(operation.definition.contract.id());
    requirement.set_fingerprint(operation.definition.contract.fingerprint());
    plugin_.requirements_.emplace_back(std::move(requirement), operation.definition.types);
    return *this;
  }
  template <class R> Registrar &add(const Stream<R> &stream);
  template <class R> Registrar &add(const Observer<R> &observer);
  template <class T> Registrar &add(Descriptor<T>) {
    detail::merge(plugin_.messages_, detail::types(T::descriptor()));
    return *this;
  }
  template <class T> Registrar &add(const Contribution<T> &contribution) {
    Section section;
    section.set_type(T::descriptor()->full_name());
    section.set_payload(detail::encode(contribution.value));
    plugin_.sections_.emplace_back(std::move(section), detail::types(T::descriptor()));
    return *this;
  }
  Registrar &add(SettingDeclaration declaration);

private:
  friend class Plugin;
  explicit Registrar(Plugin &plugin) : plugin_(plugin) {}
  Plugin &plugin_;
};

inline Plugin &Plugin::add(std::function<void(Registrar &)> catalog) {
  Registrar ctx(*this);
  ctx.add(std::move(catalog));
  return *this;
}

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
  friend class Registrar;
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
  Operation &lane(const Lane &lane) {
    definition.contract.set_lane(lane.declaration().name());
    return *this;
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
  return Watching<R>(open_watch(operation.definition, detail::encode(request), max_item_bytes));
}

template <class Q, class R>
Call<R> Context::call(const Operation<Q, R> &operation, const Q &request, std::optional<OperationPolicy> policy) {
  if (operation.definition.watch)
    throw std::invalid_argument("Call " + operation.definition.contract.id() + ": Use Watch for a Streaming Contract");
  const auto payload = detail::encode(request);
  std::function<void()> cancel;
  if (auto result = local(operation.definition, payload, policy, &cancel))
    return Call<R>(*result, std::move(cancel));
  auto result = send(operation.definition, payload, std::move(policy), &cancel);
  return Call<R>(std::move(result), std::move(cancel));
}

template <class R> class Publisher {
public:
  explicit Publisher(std::function<void(std::string, std::optional<google::protobuf::Timestamp>)> send) : send_(std::move(send)) {}
  void publish(const R &row, std::optional<google::protobuf::Timestamp> at = {}) const {
    send_(detail::encode(row), std::move(at));
  }

private:
  std::function<void(std::string, std::optional<google::protobuf::Timestamp>)> send_;
};

template <class R> class Stream {
private:
  friend class Plugin;
  friend class Registrar;
  friend class Observer<R>;
  detail::StreamDefinition definition;

public:
  explicit Stream(std::string id) {
    definition.declaration.set_id(std::move(id));
    definition.declaration.set_row(R::descriptor()->full_name());
    definition.types = detail::types(R::descriptor());
    definition.row = R::descriptor();
  }
  Stream &publish_on(std::function<void(Context &, Publisher<R>)> producer) {
    definition.producer = [producer](Context &ctx, std::function<void(std::string, std::optional<google::protobuf::Timestamp>)> send) {
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
  friend class Registrar;
  detail::StreamDefinition definition;
  google::protobuf::FileDescriptorSet columns_;

  explicit Observer(const Stream<R> &stream) : definition(stream.definition) {
    detail::validate_row(R::descriptor());
  }

public:
  // Answers One Sample per Call; the SDK Calls It for Each Sample of a Window in Order.
  Observer(const Stream<R> &stream, std::function<R(Context &)> handler) : Observer(stream) {
    definition.sample = [handler](Context &ctx) -> std::unique_ptr<google::protobuf::Message> {
      return std::make_unique<R>(handler(ctx));
    };
  }
  // Answers a Whole Window; C Repeats Every Field of R with the Same Number and Type.
  template <class C> static Observer window(const Stream<R> &stream, std::function<C(Context &, const Window &)> handler) {
    static_assert(std::is_base_of_v<google::protobuf::Message, C>);
    Observer observer(stream);
    detail::validate_columns(R::descriptor(), C::descriptor());
    observer.columns_ = detail::types(C::descriptor());
    observer.definition.window = [handler](Context &ctx, const Window &grant) -> std::unique_ptr<google::protobuf::Message> {
      return std::make_unique<C>(handler(ctx, grant));
    };
    return observer;
  }
  Observer &every(const google::protobuf::Duration &interval) {
    *definition.declaration.mutable_interval() = interval;
    return *this;
  }
  Observer &interval_setting(std::string id) {
    definition.declaration.set_interval_setting(std::move(id));
    return *this;
  }
  // Names a Plugin Duration Setting That Overrides the Host's Window Length for This Stream.
  Observer &window_setting(std::string id) {
    definition.declaration.set_window_setting(std::move(id));
    return *this;
  }
};

template <class R> Registrar &Registrar::add(const Stream<R> &stream) {
  plugin_.streams_.push_back(stream.definition);
  return *this;
}
template <class R> Registrar &Registrar::add(const Observer<R> &observer) {
  for (auto &stream : plugin_.streams_) {
    if (stream.declaration.id() != observer.definition.declaration.id())
      continue;
    if (stream.sample || stream.window || stream.producer || observer.definition.producer)
      throw std::invalid_argument("Duplicate Stream Producer");
    const bool same_row = stream.declaration.row() == observer.definition.declaration.row();
    const bool same_types = detail::encode(stream.types) == detail::encode(observer.definition.types);
    if (!same_row || !same_types)
      throw std::invalid_argument("Observer Row Differs from Its Stream");
    stream.sample = observer.definition.sample;
    stream.window = observer.definition.window;
    if (observer.definition.declaration.has_interval()) {
      *stream.declaration.mutable_interval() = observer.definition.declaration.interval();
    }
    stream.declaration.set_interval_setting(observer.definition.declaration.interval_setting());
    stream.declaration.set_window_setting(observer.definition.declaration.window_setting());
    detail::merge(stream.types, observer.columns_);
    return *this;
  }
  throw std::invalid_argument("Observer Requires a Declared Stream");
}

/* === Public Declarations === */

} // namespace harmos::plugin

#include "harmos/_generated/host.hpp"
