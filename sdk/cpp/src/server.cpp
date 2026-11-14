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
#include <cstdlib>
#include <deque>
#include <iostream>
#include <thread>

// Third Party Imports
#include <grpcpp/grpcpp.h>

// Local Imports
#include "duplex.hpp"
#include "harmos/plugin/v1/instance.grpc.pb.h"
#include "invocation.hpp"
#include "samples.hpp"

namespace chrono = std::chrono;
using grpc::CallbackServerContext;
using grpc::InsecureServerCredentials;
using grpc::ServerBidiReactor;
using grpc::ServerBuilder;
using grpc::ServerUnaryReactor;
using grpc::Status;
using grpc::StatusCode;
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
namespace wire = ::harmos::model::v1;
namespace {
using detail::Duplex;
using Requirements = std::map<std::string, std::string>;

/* === Classes === */

class InvocationCall final : public Duplex<plugin::v1::InstanceInvokeRequest, plugin::v1::InstanceInvokeResponse> {
public:
  InvocationCall(const vector<detail::OperationDefinition> &operations, const Requirements &requirements, const string &plugin)
      : operations_(operations), state_(std::make_shared<detail::Invocation>()) {
    state_->requirements = requirements;
    for (const auto &operation : operations)
      if (operation.contract.visibility() == wire::VISIBILITY_PRIVATE)
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
        if (candidate.contract.visibility() != wire::VISIBILITY_PRIVATE && candidate.contract.id() == input.request().operation())
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
      wire::Failure failure;
      failure.set_classification(wire::CLASSIFICATION_CANCELLED);
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
  Requirements requirements;
  vector<std::function<void()>> cleanup;
  std::function<void()> stop;

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
    return new detail::PublicationCall(streams, operations, requirements, description.id());
  }
  ServerUnaryReactor *Stop(CallbackServerContext *context, const plugin::v1::InstanceStopRequest *,
                           plugin::v1::InstanceStopResponse *) override {
    try {
      for (const auto &hook : cleanup)
        hook();
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

void Plugin::serve(string address) const {
  const bool remote = !address.empty();
  if (address.empty()) {
#ifdef _WIN32
    char *raw = nullptr;
    size_t length = 0;
    if (_dupenv_s(&raw, &length, "HARMOS_ADDRESS") != 0)
      throw runtime_error("Cannot Read HARMOS_ADDRESS");
    const unique_ptr<char, decltype(&std::free)> configured(raw, &std::free);
    address = configured ? configured.get() : "127.0.0.1:0";
#else
    const char *configured = std::getenv("HARMOS_ADDRESS");
    address = configured ? configured : "127.0.0.1:0";
#endif
  }
  Service service;
  auto &description = service.description;
  description.set_id(id_);
  description.set_version(version_);
  description.set_revision(1);
  description.set_sdk("harmos_cpp");
  const auto maximum = transfer_message_bytes(transfer_);
  description.set_sdk_version("0.1.0");
  description.set_message_bytes(maximum);
  *description.mutable_declarations() = declarations();
  static const auto started = chrono::system_clock::now().time_since_epoch();
  const auto seconds = chrono::duration_cast<chrono::seconds>(started);
  description.mutable_started_at()->set_seconds(seconds.count());
  const auto nanos = chrono::duration_cast<chrono::nanoseconds>(started - seconds);
  description.mutable_started_at()->set_nanos(static_cast<int>(nanos.count()));
  service.operations = operations_;
  service.streams = streams_;
  service.cleanup = cleanup_;
  for (const auto &[requirement, types] : requirements_)
    service.requirements[requirement.id()] = requirement.fingerprint();
  mutex mutex;
  condition_variable changed;
  bool stopped = false;
  service.stop = [&] {
    if (remote)
      return;
    lock_guard lock(mutex);
    stopped = true;
    changed.notify_all();
  };
  ServerBuilder builder;
  builder.SetMaxReceiveMessageSize(maximum);
  builder.SetMaxSendMessageSize(maximum);
  int port = 0;
  builder.AddListeningPort(address, InsecureServerCredentials(), &port);
  builder.RegisterService(&service);
  auto server = builder.BuildAndStart();
  if (!server || port == 0)
    throw runtime_error("Cannot Bind Plugin Address");
  const auto colon = address.rfind(':');
  std::cout << "harmos listening " << address.substr(0, colon + 1) << port << std::endl;
  if (remote) {
    server->Wait();
    return;
  }
  {
    unique_lock lock(mutex);
    changed.wait(lock, [&] {
      return stopped;
    });
  }
  server->Shutdown();
  server->Wait();
}
} // namespace harmos::plugin

/* === Private Functions === */

/* === Entry Point === */
