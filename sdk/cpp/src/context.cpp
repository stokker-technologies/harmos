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
#include "invocation.hpp"

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

namespace harmos {
Context::Context(std::shared_ptr<detail::Invocation> state) : state_(std::move(state)) {
    invocation = state_->request.invocation();
    scope_ = state_->request.scope();
    attempt = state_->request.attempt();
}

const std::string& Context::scope() const {
    if (scope_.empty()) {
        wire::Failure failure;
        failure.set_classification(wire::CLASSIFICATION_REFUSED);
        failure.set_code("scope_required");
        failure.set_message("Invocation Scope Is Required: the caller must bind a scope");
        throw Failure(std::move(failure));
    }
    return scope_;
}

std::shared_future<std::string> Context::send(wire::ChildRequest request) {
    std::shared_future<std::string> result;
    {
        std::lock_guard lock(state_->mutex);
        if (!state_->emit) throw std::logic_error("Observers and Publishers Cannot Call Operations");
        if (state_->closed) throw TransportError(14, "Invocation Closed");
        auto found = state_->requirements.find(request.operation());
        if (found == state_->requirements.end() || found->second != request.fingerprint()) {
            throw std::invalid_argument("Undeclared Requirement");
        }
        request.set_call(++state_->next);
        auto& promise = state_->pending[request.call()];
        result = promise.get_future().share();
    }
    plugin::v1::InstanceInvokeResponse frame;
    *frame.mutable_child() = std::move(request);
    state_->emit(std::move(frame));
    state_->start_reading();
    return result;
}

std::string Context::cancelled() {
    state_->start_reading();
    std::unique_lock lock(state_->mutex);
    state_->changed.wait(lock, [&] { return state_->cancelled.has_value(); });
    return *state_->cancelled;
}

void Context::sleep(const google::protobuf::Duration& duration) {
    context::v1::TimeSleepRequest request;
    *request.mutable_duration() = duration;
    call(SLEEP, request).get();
}

google::protobuf::Timestamp Context::now() { return call(GET_TIME, context::v1::TimeGetRequest{}).get().now(); }
}

/* === Private Functions === */

/* === Entry Point === */
