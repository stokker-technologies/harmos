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
#include "invocation.hpp"
#include "duplex.hpp"
#include "samples.hpp"
#include "harmos/plugin/v1/instance.grpc.pb.h"

/* === Macros === */

/* === Constants === */

/* === Types === */

namespace harmos {
namespace {
using detail::Duplex;
using Requirements = std::map<std::string, std::string>;

/* === Classes === */

class InvocationCall final : public Duplex<plugin::v1::InstanceInvokeRequest, plugin::v1::InstanceInvokeResponse> {
public:
    InvocationCall(const std::vector<detail::OperationDefinition>& operations, const Requirements& requirements, const std::string& plugin)
        : operations_(operations), state_(std::make_shared<detail::Invocation>()) {
        state_->requirements = requirements;
        for (const auto& operation : operations) if (operation.contract.visibility() == wire::VISIBILITY_PRIVATE)
            state_->local.emplace(operation.contract.id(), operation);
        state_->plugin = plugin;
        state_->emit = [this](plugin::v1::InstanceInvokeResponse frame) { write(std::move(frame)); };
        state_->read = [this] { read_next(); };
        read_next();
    }

    void settle_children() override { state_->settle_workers(); }
    void closed() override {
        state_->close(std::make_exception_ptr(TransportError(14, "Transport Closed")));
    }

    void OnReadDone(bool ok) override {
        if (!ok) {
            if (!started_) { end({grpc::StatusCode::INVALID_ARGUMENT, "Request Must Be First"}); return; }
            bool pending;
            { std::lock_guard lock(state_->mutex); pending = !state_->pending.empty() || !state_->watches.empty(); }
            if (pending) { closed(); end({grpc::StatusCode::UNAVAILABLE, "Host Closed Before Child Replies"}); }
            return;
        }
        if (!started_) {
            if (!input.has_request() || input.request().invocation().empty() || input.request().attempt() == 0) {
                end({grpc::StatusCode::INVALID_ARGUMENT, "Invocation and Attempt Are Required"});
                return;
            }
            state_->request = input.request();
            const detail::OperationDefinition* operation = nullptr;
            for (const auto& candidate : operations_) if (candidate.contract.visibility() != wire::VISIBILITY_PRIVATE && candidate.contract.id() == input.request().operation()) operation = &candidate;
            if (!operation) { end({grpc::StatusCode::NOT_FOUND, "Unknown Operation"}); return; }
            started_ = true;
            worker = std::thread([this, operation] {
                plugin::v1::InstanceInvokeResponse frame;
                try {
                    Context context(state_);
                    frame.mutable_response()->set_payload(operation->handler(context, state_->request.payload()));
                } catch (const Failure& failure) {
                    *frame.mutable_response()->mutable_failure() = failure.value;
                } catch (const TransportError& error) {
                    end({static_cast<grpc::StatusCode>(error.code), error.what()});
                    return;
                } catch (const std::exception& error) {
                    end({grpc::StatusCode::INTERNAL, error.what()});
                    return;
                }
                try { write(std::move(frame)); } catch (const TransportError&) { return; }
                end(grpc::Status::OK);
            });
            return;
        }
        if (input.has_child()) {
            const bool valid = state_->receive(input.child());
            if (!valid) {
                end({grpc::StatusCode::INVALID_ARGUMENT, "Unknown or Duplicate Child Response"});
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
            std::vector<std::uint64_t> pending;
            {
                std::lock_guard lock(state_->mutex);
                for (const auto& [id, promise] : state_->pending) if (!state_->guards.contains(id)) pending.push_back(id);
                for (const auto& [id, watch] : state_->watches) pending.push_back(id);
            }
            // Child Cancellation Precedes Waking the Handler's Terminal Failure.
            for (auto id : pending) {
                plugin::v1::InstanceInvokeResponse frame;
                frame.mutable_child_cancel()->set_call(id);
                try { write(std::move(frame)); } catch (const TransportError&) { return; }
            }
            {
                std::lock_guard lock(state_->mutex);
                if (!state_->cancelled) state_->cancelled = input.cancel().reason();
                state_->changed.notify_all();
            }
            {
                std::lock_guard lock(state_->mutex);
                for (auto id : pending) {
                    auto found = state_->pending.find(id);
                    if (found != state_->pending.end()) { found->second.set_exception(std::make_exception_ptr(Failure(failure))); state_->pending.erase(found); }
                    auto watching = state_->watches.find(id);
                    if (watching != state_->watches.end()) {
                        wire::ChildResponse terminal; terminal.set_call(id); *terminal.mutable_failure() = failure;
                        watching->second->terminal = std::move(terminal); state_->watches.erase(watching);
                    }
                    state_->retired.insert(id);
                }
                state_->changed.notify_all();
            }
        } else {
            // Keep the Protocol Error Authoritative When Handler Cleanup Wakes.
            end({grpc::StatusCode::INVALID_ARGUMENT, "Repeated Request or Unset Frame"});
            closed();
            return;
        }
        read_next();
    }
private:
    const std::vector<detail::OperationDefinition>& operations_;
    std::shared_ptr<detail::Invocation> state_;
    bool started_ = false;
};

class Stopped final : public grpc::ServerUnaryReactor {
public:
    explicit Stopped(std::function<void()> done) : done_(std::move(done)) { Finish(grpc::Status::OK); }
    void OnDone() override { done_(); delete this; }
private:
    std::function<void()> done_;
};

class Service final : public plugin::v1::Instance::CallbackService {
public:
    plugin::v1::InstanceGetResponse description;
    std::vector<detail::OperationDefinition> operations;
    std::vector<detail::StreamDefinition> streams;
    Requirements requirements;
    std::vector<std::function<void()>> cleanup;
    std::function<void()> stop;

    grpc::ServerUnaryReactor* Get(grpc::CallbackServerContext* context, const plugin::v1::InstanceGetRequest* request,
                                      plugin::v1::InstanceGetResponse* response) override {
        auto* reactor = context->DefaultReactor();
        if (request->revision() == 0) {
            reactor->Finish({grpc::StatusCode::INVALID_ARGUMENT, "Revision Must Be at Least One"});
            return reactor;
        }
        *response = description;
        reactor->Finish(grpc::Status::OK);
        return reactor;
    }
    grpc::ServerBidiReactor<plugin::v1::InstanceInvokeRequest, plugin::v1::InstanceInvokeResponse>* Invoke(grpc::CallbackServerContext*) override {
        return new InvocationCall(operations, requirements, description.id());
    }
    grpc::ServerBidiReactor<plugin::v1::InstanceSamplesRequest, plugin::v1::InstanceSamplesResponse>* Samples(grpc::CallbackServerContext*) override {
        return new detail::PublicationCall(streams, operations, requirements, description.id());
    }
    grpc::ServerUnaryReactor* Stop(grpc::CallbackServerContext* context, const plugin::v1::InstanceStopRequest*, plugin::v1::InstanceStopResponse*) override {
        try { for (const auto& hook : cleanup) hook(); }
        catch (const std::exception& error) {
            auto* reactor = context->DefaultReactor();
            reactor->Finish({grpc::StatusCode::INTERNAL, error.what()});
            return reactor;
        }
        return new Stopped(stop);
    }
};
}

/* === Templates === */

/* === Public Functions === */

void Plugin::serve(std::string address) const {
    const bool remote = !address.empty();
    if (address.empty()) {
#ifdef _WIN32
        char* raw = nullptr;
        std::size_t length = 0;
        if (_dupenv_s(&raw, &length, "HARMOS_ADDRESS") != 0) throw std::runtime_error("Cannot Read HARMOS_ADDRESS");
        const std::unique_ptr<char, decltype(&std::free)> configured(raw, &std::free);
        address = configured ? configured.get() : "127.0.0.1:0";
#else
        const char* configured = std::getenv("HARMOS_ADDRESS");
        address = configured ? configured : "127.0.0.1:0";
#endif
    }
    Service service;
    auto& description = service.description;
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
    for (const auto& [requirement, types] : requirements_) service.requirements[requirement.id()] = requirement.fingerprint();
    std::mutex mutex;
    std::condition_variable changed;
    bool stopped = false;
    service.stop = [&] {
        if (remote) return;
        std::lock_guard lock(mutex);
        stopped = true;
        changed.notify_all();
    };
    grpc::ServerBuilder builder;
    builder.SetMaxReceiveMessageSize(maximum);
    builder.SetMaxSendMessageSize(maximum);
    int port = 0;
    builder.AddListeningPort(address, grpc::InsecureServerCredentials(), &port);
    builder.RegisterService(&service);
    auto server = builder.BuildAndStart();
    if (!server || port == 0) throw std::runtime_error("Cannot Bind Plugin Address");
    const auto colon = address.rfind(':');
    std::cout << "harmos listening " << address.substr(0, colon + 1) << port << std::endl;
    if (remote) { server->Wait(); return; }
    {
        std::unique_lock lock(mutex);
        changed.wait(lock, [&] { return stopped; });
    }
    server->Shutdown();
    server->Wait();
}
}

/* === Private Functions === */

/* === Entry Point === */
