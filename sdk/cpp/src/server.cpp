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
#include <chrono>
#include <condition_variable>
#include <cstdlib>
#include <exception>
#include <functional>
#include <iostream>
#include <map>
#include <mutex>
#include <thread>

// Third Party Imports
#include <grpc/event_engine/event_engine.h>
#include <grpcpp/grpcpp.h>

// Local Imports
#include "duplex.hpp"
#include "harmos/plugin/v1/instance.grpc.pb.h"
#include "invocation.hpp"
#include "samples.hpp"

using grpc::CallbackServerContext;
using grpc::ChannelArguments;
using grpc::InsecureServerCredentials;
using grpc::ServerBidiReactor;
using grpc::ServerBuilder;
using grpc::ServerBuilderOption;
using grpc::ServerBuilderPlugin;
using grpc::ServerUnaryReactor;
using grpc::Status;
using grpc::StatusCode;
using grpc_event_engine::experimental::EventEngine;
using std::condition_variable;
using std::exception;
using std::lock_guard;
using std::mutex;
using std::runtime_error;
using std::shared_ptr;
using std::size_t;
using std::string;
using std::thread;
using std::uint64_t;
using std::unique_lock;
using std::unique_ptr;
using std::vector;

/* === Macros === */

/* === Constants === */

/* === Types === */

namespace harmos::plugin {
namespace {
using detail::Duplex;

/* === Classes === */

// gRPC 1.76 Accepts a Per-Server Engine Through Owned Channel Arguments.
class ServingEngine final : public ServerBuilderOption {
public:
  explicit ServingEngine(shared_ptr<EventEngine> engine) : engine_(std::move(engine)) {}

  void UpdateArguments(ChannelArguments *arguments) override {
    static const grpc_arg_pointer_vtable ownership = {
        [](void *value) -> void * {
          return new shared_ptr<EventEngine>(*static_cast<shared_ptr<EventEngine> *>(value));
        },
        [](void *value) {
          delete static_cast<shared_ptr<EventEngine> *>(value);
        },
        [](void *left, void *right) {
          const auto *left_engine = static_cast<shared_ptr<EventEngine> *>(left)->get();
          const auto *right_engine = static_cast<shared_ptr<EventEngine> *>(right)->get();
          const std::less<const EventEngine *> before;
          return before(left_engine, right_engine) ? -1 : before(right_engine, left_engine) ? 1 : 0;
        }};
    arguments->SetPointerWithVtable("grpc.internal.event_engine", &engine_, &ownership);
  }

  void UpdatePlugins(vector<unique_ptr<ServerBuilderPlugin>> *) override {}

private:
  shared_ptr<EventEngine> engine_;
};

class InvocationCall final : public Duplex<plugin::v1::InstanceInvokeRequest, plugin::v1::InstanceInvokeResponse> {
public:
  InvocationCall(const vector<detail::OperationDefinition> &operations, const std::map<string, string> &requirements, const string &plugin)
      : operations_(operations), state_(std::make_shared<detail::Invocation>()) {
    state_->requirements = requirements;
    for (const auto &operation : operations)
      if (operation.contract.visibility() == model::v1::VISIBILITY_PRIVATE)
        state_->local.emplace(operation.contract.id(), operation);
    state_->plugin = plugin;
    state_->emit = [this](plugin::v1::InstanceInvokeResponse frame) {
      write(std::move(frame));
    };
    state_->read = [this] {
      read_next();
    };
    read_next();
  }

  void settle_children() override {
    state_->settle_workers();
  }
  void closed() override {
    state_->close(std::make_exception_ptr(TransportError(14, "Transport Closed")));
  }

  void OnReadDone(bool ok) override {
    if (!ok) {
      if (!started_) {
        end({StatusCode::INVALID_ARGUMENT, "Request Must Be First"});
        return;
      }
      bool pending;
      {
        lock_guard lock(state_->mutex);
        pending = !state_->pending.empty() || !state_->watches.empty();
      }
      if (pending) {
        closed();
        end({StatusCode::UNAVAILABLE, "Host Closed Before Child Replies"});
      }
      return;
    }
    if (!started_) {
      if (!input.has_request() || input.request().invocation().empty() || input.request().attempt() == 0) {
        end({StatusCode::INVALID_ARGUMENT, "Invocation and Attempt Are Required"});
        return;
      }
      state_->request = input.request();
      const detail::OperationDefinition *operation = nullptr;
      for (const auto &candidate : operations_)
        if (candidate.contract.visibility() != model::v1::VISIBILITY_PRIVATE && candidate.contract.id() == input.request().operation())
          operation = &candidate;
      if (!operation) {
        end({StatusCode::NOT_FOUND, "Unknown Operation"});
        return;
      }
      started_ = true;
      worker = thread([this, operation] {
        plugin::v1::InstanceInvokeResponse frame;
        try {
          Context context(state_);
          frame.mutable_response()->set_payload(operation->handler(context, state_->request.payload()));
        } catch (const Failure &failure) {
          *frame.mutable_response()->mutable_failure() = failure.value;
        } catch (const TransportError &error) {
          end({static_cast<StatusCode>(error.code), error.what()});
          return;
        } catch (const exception &error) {
          end({StatusCode::INTERNAL, error.what()});
          return;
        }
        try {
          write(std::move(frame));
        } catch (const TransportError &) {
          return;
        }
        end(Status::OK);
      });
      return;
    }
    if (input.has_child()) {
      const bool valid = state_->receive(input.child());
      if (!valid) {
        end({StatusCode::INVALID_ARGUMENT, "Unknown or Duplicate Child Response"});
        closed();
        return;
      }
    } else if (input.has_cancel()) {
      model::v1::Failure failure;
      failure.set_classification(model::v1::CLASSIFICATION_CANCELLED);
      failure.set_code("cancelled");
      failure.set_message(input.cancel().reason());
      failure.set_action("Retry If Needed");
      failure.set_operation(state_->request.operation());
      failure.set_plugin(state_->plugin);
      vector<uint64_t> pending;
      {
        lock_guard lock(state_->mutex);
        for (const auto &[id, promise] : state_->pending)
          if (!state_->guards.contains(id))
            pending.push_back(id);
        for (const auto &[id, watch] : state_->watches)
          pending.push_back(id);
      }
      // Child Cancellation Precedes Waking the Handler's Terminal Failure.
      for (auto id : pending) {
        plugin::v1::InstanceInvokeResponse frame;
        frame.mutable_child_cancel()->set_call(id);
        try {
          write(std::move(frame));
        } catch (const TransportError &) {
          return;
        }
      }
      {
        lock_guard lock(state_->mutex);
        if (!state_->cancelled)
          state_->cancelled = input.cancel().reason();
        state_->changed.notify_all();
      }
      {
        lock_guard lock(state_->mutex);
        for (auto id : pending) {
          auto found = state_->pending.find(id);
          if (found != state_->pending.end()) {
            found->second.set_exception(std::make_exception_ptr(Failure(failure)));
            state_->pending.erase(found);
          }
          auto watching = state_->watches.find(id);
          if (watching != state_->watches.end()) {
            plugin::v1::ChildResponse terminal;
            terminal.set_call(id);
            *terminal.mutable_failure() = failure;
            watching->second->terminal = std::move(terminal);
            state_->watches.erase(watching);
          }
          state_->retired.insert(id);
        }
        state_->changed.notify_all();
      }
    } else {
      // Keep the Protocol Error Authoritative When Handler Cleanup Wakes.
      end({StatusCode::INVALID_ARGUMENT, "Repeated Request or Unset Frame"});
      closed();
      return;
    }
    read_next();
  }

private:
  const vector<detail::OperationDefinition> &operations_;
  shared_ptr<detail::Invocation> state_;
  bool started_ = false;
};

class Stopped final : public ServerUnaryReactor {
public:
  explicit Stopped(std::function<void()> done) : done_(std::move(done)) {
    Finish(Status::OK);
  }
  void OnDone() override {
    done_();
    delete this;
  }

private:
  std::function<void()> done_;
};

class Service final : public plugin::v1::Instance::CallbackService {
public:
  plugin::v1::InstanceGetResponse description;
  vector<detail::OperationDefinition> operations;
  vector<detail::StreamDefinition> streams;
  std::map<string, string> requirements;
  vector<std::function<void()>> cleanup;
  std::function<void()> stop;
  std::once_flag cleanup_once;
  std::exception_ptr cleanup_failure;

  void finish_cleanup() {
    std::call_once(cleanup_once, [this] {
      for (const auto &hook : cleanup) {
        try {
          hook();
        } catch (...) {
          if (!cleanup_failure)
            cleanup_failure = std::current_exception();
        }
      }
    });
    if (cleanup_failure)
      std::rethrow_exception(cleanup_failure);
  }

  ServerUnaryReactor *Get(CallbackServerContext *context, const plugin::v1::InstanceGetRequest *request,
                          plugin::v1::InstanceGetResponse *response) override {
    auto *reactor = context->DefaultReactor();
    if (request->revision() == 0) {
      reactor->Finish({StatusCode::INVALID_ARGUMENT, "Revision Must Be at Least One"});
      return reactor;
    }
    *response = description;
    reactor->Finish(Status::OK);
    return reactor;
  }
  ServerBidiReactor<plugin::v1::InstanceInvokeRequest, plugin::v1::InstanceInvokeResponse> *Invoke(CallbackServerContext *) override {
    return new InvocationCall(operations, requirements, description.id());
  }
  ServerBidiReactor<plugin::v1::InstanceSamplesRequest, plugin::v1::InstanceSamplesResponse> *Samples(CallbackServerContext *) override {
    return new detail::PublicationCall(streams, operations, requirements, description.id(), description.message_bytes());
  }
  ServerUnaryReactor *Stop(CallbackServerContext *context, const plugin::v1::InstanceStopRequest *,
                           plugin::v1::InstanceStopResponse *) override {
    try {
      finish_cleanup();
    } catch (const exception &error) {
      auto *reactor = context->DefaultReactor();
      reactor->Finish({StatusCode::INTERNAL, error.what()});
      return reactor;
    }
    return new Stopped(stop);
  }
};
} // namespace

/* === Templates === */

/* === Public Functions === */

struct Listener::State {
  Service service;
  shared_ptr<EventEngine> engine;
  unique_ptr<grpc::Server> server;
  string address;
  mutex stop_mutex;
  condition_variable changed;
  bool stopped = false;
  thread worker;
  std::exception_ptr failure;

  void request_stop() {
    lock_guard lock(stop_mutex);
    stopped = true;
    changed.notify_all();
  }

  void wait() {
    if (worker.joinable())
      worker.join();
    if (failure)
      std::rethrow_exception(std::exchange(failure, {}));
  }

  ~State() {
    request_stop();
    try {
      wait();
    } catch (...) {
      // Explicit Close Reports Cleanup Failure; Destruction Still Releases Resources.
    }
    if (server) {
      server->Shutdown();
      server->Wait();
    }
  }
};

Listener::Listener(unique_ptr<State> state) : state_(std::move(state)) {}
Listener::Listener(Listener &&) noexcept = default;
Listener &Listener::operator=(Listener &&) noexcept = default;
Listener::~Listener() = default;

const string &Listener::address() const {
  if (!state_)
    throw std::logic_error("Read Plugin Address: Use a Listener That Has Not Been Moved");
  return state_->address;
}

void Listener::wait() {
  if (state_)
    state_->wait();
}

void Listener::close() {
  if (!state_)
    return;
  state_->request_stop();
  wait();
}

Listener Plugin::start(string address) const {
  return start(std::move(address), true);
}

Listener Plugin::start(string address, bool remote) const {
  auto state = std::make_unique<Listener::State>();
  auto &service = state->service;
  auto &description = service.description;
  description.set_id(id_);
  description.set_version(version_);
  description.set_revision(1);
  description.set_sdk("harmos_cpp");
  const auto maximum = transfer_message_bytes(transfer_);
  description.set_sdk_version("0.1.0");
  description.set_message_bytes(maximum);
  *description.mutable_declarations() = declarations();
  static const auto started = std::chrono::system_clock::now().time_since_epoch();
  const auto seconds = std::chrono::duration_cast<std::chrono::seconds>(started);
  description.mutable_started_at()->set_seconds(seconds.count());
  const auto nanos = std::chrono::duration_cast<std::chrono::nanoseconds>(started - seconds);
  description.mutable_started_at()->set_nanos(static_cast<int>(nanos.count()));
  service.operations = operations_;
  service.streams = streams_;
  service.cleanup = cleanup_;
  for (const auto &[requirement, types] : requirements_)
    service.requirements[requirement.id()] = requirement.fingerprint();
  service.stop = [owned = state.get(), remote] {
    if (!remote)
      owned->request_stop();
  };
  ServerBuilder builder;
  state->engine = grpc_event_engine::experimental::CreateEventEngine();
  builder.SetOption(std::make_unique<ServingEngine>(state->engine));
  builder.SetMaxReceiveMessageSize(maximum);
  builder.SetMaxSendMessageSize(maximum);
  int port = 0;
  builder.AddListeningPort(address, InsecureServerCredentials(), &port);
  builder.RegisterService(&service);
  state->server = builder.BuildAndStart();
  if (!state->server || port == 0)
    throw runtime_error("Start Plugin: Choose an Available Listening Address: " + address);
  const auto colon = address.rfind(':');
  state->address = address.substr(0, colon + 1) + std::to_string(port);
  state->worker = thread([owned = state.get()] {
    {
      unique_lock lock(owned->stop_mutex);
      owned->changed.wait(lock, [owned] {
        return owned->stopped;
      });
    }
    try {
      owned->service.finish_cleanup();
    } catch (...) {
      owned->failure = std::current_exception();
    }
    owned->server->Shutdown();
    owned->server->Wait();
    owned->server.reset();
    owned->engine.reset();
  });
  return Listener(std::move(state));
}

void Plugin::run(string address) const {
  const bool remote = !address.empty();
#ifdef _WIN32
  char *raw = nullptr;
  size_t length = 0;
  if (_dupenv_s(&raw, &length, "HARMOS_ADDRESS") != 0)
    throw runtime_error("Read Plugin Address: Check HARMOS_ADDRESS in the Process Environment");
  const unique_ptr<char, decltype(&std::free)> configured(raw, &std::free);
  if (address.empty())
    address = configured ? configured.get() : "127.0.0.1:0";
#else
  const char *configured = std::getenv("HARMOS_ADDRESS");
  if (address.empty())
    address = configured ? configured : "127.0.0.1:0";
#endif
  auto listener = start(std::move(address), remote);
  if (configured)
    std::cout << "harmos listening " << listener.address() << std::endl;
  listener.wait();
}
} // namespace harmos::plugin

/* === Private Functions === */

/* === Entry Point === */
