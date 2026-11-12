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
#include "harmos/plugin/v1/instance.grpc.pb.h"

/* === Macros === */

/* === Constants === */

/* === Types === */

namespace harmos {
namespace {
using Requirements = std::map<std::string, std::string>;

/* === Classes === */

// Each Call Owns Its Handler and One Flow-Controlled Outgoing Message.
template<class I, class O> class Duplex : public grpc::ServerBidiReactor<I, O> {
public:
    Duplex() { this->StartSendInitialMetadata(); }
    I input;
    std::thread worker;
    std::recursive_mutex mutex;
    std::condition_variable_any room;
    std::deque<O> output;
    bool ending = false;
    bool finishing = false;
    bool metadata_sent = false;
    bool read_requested = false;
    grpc::Status status;

    void read_next() {
        std::lock_guard lock(mutex);
        if (ending) return;
        if (metadata_sent) this->StartRead(&input);
        else read_requested = true;
    }
    void OnSendInitialMetadataDone(bool ok) override {
        std::lock_guard lock(mutex);
        if (!ok) {
            closed();
            end({grpc::StatusCode::CANCELLED, "Transport Closed"});
            return;
        }
        metadata_sent = true;
        if (read_requested && !ending) this->StartRead(&input);
    }
    void write(O message) {
        std::unique_lock lock(mutex);
        room.wait(lock, [&] { return output.empty() || ending; });
        if (ending) throw TransportError(1, "Call Closed");
        output.push_back(std::move(message));
        this->StartWrite(&output.front());
    }
    void end(grpc::Status result) {
        std::lock_guard lock(mutex);
        if (ending) return;
        ending = true;
        status = std::move(result);
        room.notify_all();
        if (output.empty()) {
            finishing = true;
            this->Finish(status);
        }
    }
    void OnWriteDone(bool ok) override {
        std::lock_guard lock(mutex);
        output.pop_front();
        room.notify_all();
        if (!ok && !ending) {
            closed();
            end(grpc::Status(grpc::StatusCode::CANCELLED, "Transport Closed"));
        }
        if (ending && !finishing) {
            finishing = true;
            this->Finish(status);
        }
    }
    void OnCancel() override {
        closed();
        end(grpc::Status(grpc::StatusCode::CANCELLED, "Transport Closed"));
    }
    void OnDone() override {
        closed();
        if (worker.joinable()) worker.join();
        settle_children();
        delete this;
    }
    virtual void closed() = 0;
    virtual void settle_children() {}
};

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
            bool valid = false;
            {
                std::lock_guard lock(state_->mutex);
                const auto& child = input.child();
                auto watching = state_->watches.find(child.call());
                if (state_->retired.contains(child.call())) { valid = true; }
                else if (watching != state_->watches.end()) {
                    auto& watch = *watching->second;
                    valid = child.result_case() != wire::ChildResponse::RESULT_NOT_SET && child.item().size() <= watch.max_item_bytes && child.payload().size() <= watch.max_item_bytes;
                    if (child.has_item()) {
                        valid = valid && watch.demanded && !watch.item && !watch.terminal;
                        if (valid) watch.item = child;
                    } else {
                        valid = valid && !watch.terminal;
                        if (valid) watch.terminal = child;
                    }
                    state_->changed.notify_all();
                } else {
                auto found = state_->pending.find(child.call());
                valid = !child.has_item() && found != state_->pending.end() && child.result_case() != wire::ChildResponse::RESULT_NOT_SET;
                if (valid) {
                    if (child.has_failure()) found->second.set_exception(std::make_exception_ptr(Failure(child.failure())));
                    else found->second.set_value(child.payload());
                    state_->guards.erase(child.call());
                    state_->pending.erase(found);
                    state_->changed.notify_all();
                }
                }
            }
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

bool valid_timestamp(const google::protobuf::Timestamp& at) {
    return at.seconds() >= -62135596800LL && at.seconds() <= 253402300799LL && at.nanos() >= 0 && at.nanos() < 1000000000;
}

class PublicationCall final : public Duplex<plugin::v1::InstanceSamplesRequest, plugin::v1::InstanceSamplesResponse> {
public:
    explicit PublicationCall(const std::vector<detail::StreamDefinition>& streams)
        : streams_(streams), state_(std::make_shared<detail::Invocation>()) {
        read_next();
    }
    void closed() override { state_->close(std::make_exception_ptr(TransportError(1, "Demand Closed"))); }
    void OnReadDone(bool ok) override {
        if (!ok) {
            end(stream_ ? grpc::Status::OK : grpc::Status(grpc::StatusCode::INVALID_ARGUMENT, "Open Must Be First"));
            closed();
            return;
        }
        if (!stream_) {
            if (!input.has_open()) { end({grpc::StatusCode::INVALID_ARGUMENT, "Open Must Be First"}); return; }
            for (const auto& stream : streams_) if (stream.declaration.id() == input.open().stream()) stream_ = &stream;
            if (!stream_) { end({grpc::StatusCode::NOT_FOUND, "Unknown Stream"}); return; }
            state_->request.set_scope(input.open().scope());
            if (stream_->producer) {
                worker = std::thread([this] {
                    try {
                        Context context(state_);
                        stream_->producer(context, [this](std::string row, std::optional<google::protobuf::Timestamp> at) {
                            if (at && !valid_timestamp(*at)) throw TransportError(3, "Invalid Device Timestamp");
                            plugin::v1::InstanceSamplesResponse samples;
                            auto* sample = samples.add_samples();
                            sample->set_row(std::move(row));
                            sample->set_sequence(++sequence_);
                            if (at) *sample->mutable_at() = std::move(*at);
                            write(std::move(samples));
                        });
                    } catch (const std::exception& error) {
                        end({grpc::StatusCode::INTERNAL, error.what()});
                    }
                });
            }
            read_next();
            return;
        }
        if (!input.has_tick() || !input.tick().has_at() || !valid_timestamp(input.tick().at()) || !stream_->observer) {
            end({grpc::StatusCode::INVALID_ARGUMENT, "Expected a Valid Observer Tick"});
            closed();
            return;
        }
        // Observers Cannot Wait; the Next Tick Is Read Only After This Sample Is Sent.
        try {
            Context context(state_);
            context.sequence = ++sequence_;
            plugin::v1::InstanceSamplesResponse samples;
            auto* sample = samples.add_samples();
            *sample->mutable_at() = input.tick().at();
            sample->set_sequence(sequence_);
            sample->set_row(stream_->observer(context));
            tick_pending_ = true;
            write(std::move(samples));
        } catch (const std::exception& error) { end({grpc::StatusCode::INTERNAL, error.what()}); }
    }
    void OnWriteDone(bool ok) override {
        Duplex::OnWriteDone(ok);
        if (tick_pending_ && ok) { tick_pending_ = false; read_next(); }
    }
private:
    const std::vector<detail::StreamDefinition>& streams_;
    const detail::StreamDefinition* stream_ = nullptr;
    std::shared_ptr<detail::Invocation> state_;
    std::uint64_t sequence_ = 0;
    bool tick_pending_ = false;
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
        return new PublicationCall(streams);
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
    description.set_sdk_version("0.1.0");
    *description.mutable_catalog() = catalog();
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
