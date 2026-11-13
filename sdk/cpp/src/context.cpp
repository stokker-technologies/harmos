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
#include <google/protobuf/util/time_util.h>
#include <absl/numeric/int128.h>

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

namespace harmos {
namespace {
class ChildWatch final : public detail::Watch {
public:
    std::shared_ptr<detail::Invocation> state;
    std::shared_ptr<detail::WatchState> watch;
    std::uint64_t call;
    bool closed = false;
    std::optional<std::string> next() override {
        if (closed) return {};
        {
            std::lock_guard lock(state->mutex);
            if (watch->demanded) throw std::logic_error("Watch: Concurrent Next Is Not Allowed");
            watch->demanded = true;
        }
        plugin::v1::InstanceInvokeResponse demand;
        demand.mutable_child_demand()->set_call(call);
        state->emit(std::move(demand));
        state->start_reading();
        std::unique_lock lock(state->mutex);
        state->changed.wait(lock, [&] { return watch->item || watch->terminal || state->closed; });
        if (watch->item) {
            auto item = std::move(*watch->item);
            watch->item.reset();
            watch->demanded = false;
            return item.item();
        }
        closed = true;
        state->watches.erase(call);
        if (watch->terminal) {
            if (watch->terminal->has_failure()) throw Failure(watch->terminal->failure());
            return {};
        }
        throw TransportError(14, "Watch: Invocation Closed");
    }
    void cancel() override {
        {
            std::lock_guard lock(state->mutex);
            if (closed) return;
            closed = true;
            state->watches.erase(call);
            state->retired.insert(call);
            state->changed.notify_all();
        }
        plugin::v1::InstanceInvokeResponse frame;
        frame.mutable_child_cancel()->set_call(call);
        try { state->emit(std::move(frame)); } catch (const TransportError&) {}
    }
};
}

std::shared_ptr<detail::Watch> Context::open_watch(wire::ChildRequest request) {
    auto result = std::make_shared<ChildWatch>();
    auto root = state_->transport ? state_->transport : state_;
    result->state = root;
    result->watch = std::make_shared<detail::WatchState>();
    result->watch->max_item_bytes = request.max_response_bytes();
    result->watch->owner = state_;
    {
        std::lock_guard lock(root->mutex);
        auto found = state_->requirements.find(request.operation());
        if (!root->emit || !request.max_response_bytes() || found == state_->requirements.end() || found->second != request.fingerprint())
            throw std::invalid_argument("Watch " + request.operation() + ": Declared Requirement and Positive Item Limit Required");
        result->call = ++root->next;
        request.set_call(result->call);
        root->watches.emplace(result->call, result->watch);
    }
    plugin::v1::InstanceInvokeResponse frame;
    *frame.mutable_child() = std::move(request);
    root->emit(std::move(frame));
    return result;
}

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

namespace {
std::shared_ptr<detail::Invocation> transport(const std::shared_ptr<detail::Invocation>& state) {
    return state->transport ? state->transport : state;
}
std::optional<std::string> cancellation(std::shared_ptr<detail::Invocation> state) {
    while (state) {
        if (state->cancelled) return state->cancelled;
        auto caller = state->caller.lock();
        if (!caller && state->transport && state->transport != state) caller = state->transport;
        if (caller == state) break;
        state = std::move(caller);
    }
    return {};
}
wire::Failure interrupted(const std::shared_ptr<detail::Invocation>& state, wire::Classification kind, const std::string& reason) {
    wire::Failure failure;
    failure.set_classification(kind); failure.set_code("interrupted");
    failure.set_message("Invoke " + state->request.operation() + ": " + reason);
    failure.set_operation(state->request.operation()); failure.set_plugin(state->plugin);
    failure.set_action("Invoke Again If Safe");
    return failure;
}
void cancel_owned(const std::shared_ptr<detail::Invocation>& state, const std::string& reason, wire::Classification kind) {
    auto root = transport(state);
    std::vector<std::uint64_t> cancelled;
    {
        std::lock_guard lock(root->mutex);
        state->cancelled = reason;
        for (auto id : state->owned) {
            auto found = root->pending.find(id);
            if (found == root->pending.end()) continue;
            found->second.set_exception(std::make_exception_ptr(Failure(interrupted(state, kind, reason))));
            root->pending.erase(found); root->guards.erase(id); root->retired.insert(id); cancelled.push_back(id);
        }
        for (auto item = root->watches.begin(); item != root->watches.end();) {
            if (item->second->owner.lock() != state) { ++item; continue; }
            wire::ChildResponse terminal; terminal.set_call(item->first); *terminal.mutable_failure() = interrupted(state, kind, reason);
            item->second->terminal = std::move(terminal);
            root->retired.insert(item->first); cancelled.push_back(item->first); item = root->watches.erase(item);
        }
        root->changed.notify_all();
    }
    for (auto id : cancelled) {
        plugin::v1::InstanceInvokeResponse frame; frame.mutable_child_cancel()->set_call(id);
        try { root->emit(std::move(frame)); } catch (const TransportError&) {}
    }
}
void validate_duration(const google::protobuf::Duration& value, const std::string& operation, bool zero = false) {
    if (value.seconds() < 0 || value.seconds() > 315576000000LL || value.nanos() < 0 || value.nanos() >= 1000000000 ||
        (!zero && !value.seconds() && !value.nanos())) throw std::invalid_argument("Invoke " + operation + ": Invalid Policy Duration");
}
google::protobuf::Duration retry_delay(const wire::RetryPolicy& retry, std::uint64_t failed_attempt) {
    auto delay = retry.initial();
    if (retry.has_maximum()) {
        for (std::uint64_t step = 0; step < failed_attempt && delay < retry.maximum(); ++step) {
            delay *= 2; if (delay > retry.maximum()) delay = retry.maximum();
        }
    }
    if (retry.deterministic_jitter()) {
        const auto nanos = absl::uint128(delay.seconds()) * 1000000000 + delay.nanos();
        const auto extra = (absl::uint128(failed_attempt) * 17 + 11) % (nanos / 4 + 1);
        google::protobuf::Duration jitter;
        jitter.set_seconds(static_cast<std::int64_t>(absl::Uint128Low64(extra / 1000000000)));
        jitter.set_nanos(static_cast<std::int32_t>(absl::Uint128Low64(extra % 1000000000)));
        delay += jitter; if (retry.has_maximum() && delay > retry.maximum()) delay = retry.maximum();
    }
    return delay;
}
}

std::optional<std::shared_future<std::string>> Context::local(const detail::OperationDefinition& requested,
    const std::string& payload, std::optional<wire::OperationPolicy> override_policy) {
    auto root = transport(state_);
    if (!root->emit) throw std::logic_error("Observers Cannot Call Operations or Wait");
    auto found = state_->local.find(requested.contract.id());
    if (found == state_->local.end()) return {};
    auto operation = found->second;
    const auto id = operation.contract.id();
    if (operation.contract.fingerprint() != requested.contract.fingerprint()) throw std::invalid_argument("Invoke " + id + ": Local Contract Differs");
    auto policy = override_policy.value_or(operation.contract.policy());
    if (policy.has_timeout()) validate_duration(policy.timeout(), id);
    if (policy.has_attempt_timeout()) validate_duration(policy.attempt_timeout(), id);
    if (policy.has_cancellation_timeout()) validate_duration(policy.cancellation_timeout(), id, true);
    const auto attempts = policy.has_retries() ? policy.retries().total_attempts() : 1;
    if (!attempts || (attempts > 1 && !operation.contract.replay_safe())) throw std::invalid_argument("Invoke " + id + ": Invalid Retry or Replay Safety");
    if (attempts > 1) {
        validate_duration(policy.retries().initial(), id);
        if (policy.retries().has_maximum()) { validate_duration(policy.retries().maximum(), id); if (policy.retries().maximum() < policy.retries().initial()) throw std::invalid_argument("Invoke " + id + ": Invalid Retry Maximum"); }
    }
    auto completion = std::make_shared<std::promise<std::string>>();
    auto result = completion->get_future().share();
    auto caller = state_;
    root->start_worker([root, caller, completion, operation, policy, attempts, payload] {
        try {
            Context parent(caller);
            auto started = policy.has_timeout() ? parent.now() : google::protobuf::Timestamp{};
            for (std::uint64_t attempt = 0; attempt < attempts; ++attempt) {
                if (attempt >= UINT32_MAX) throw std::invalid_argument("Invoke " + operation.contract.id() + ": Attempt Identity Exhausted");
                auto local = std::make_shared<detail::Invocation>();
                local->transport = root; local->caller = caller; local->request = caller->request;
                local->request.set_operation(operation.contract.id()); local->request.set_attempt(static_cast<std::uint32_t>(attempt + 1));
                local->plugin = caller->plugin; local->requirements = caller->requirements; local->local = caller->local;
                auto guard = std::make_shared<detail::Invocation>();
                guard->transport = root; guard->request = local->request; guard->plugin = local->plugin;
                guard->requirements = caller->requirements; guard->background = true;
                Context timer(guard);
                auto alarm = [&](const google::protobuf::Duration& duration) {
                    wire::ChildRequest request; request.set_operation(SLEEP.definition.contract.id()); request.set_fingerprint(SLEEP.definition.contract.fingerprint());
                    context::v1::TimeSleepRequest sleep; *sleep.mutable_duration() = duration; request.set_payload(detail::encode(sleep));
                    return timer.send(std::move(request));
                };
                try {
                    { std::lock_guard lock(root->mutex); if (auto reason = cancellation(caller)) throw Failure(interrupted(local, wire::CLASSIFICATION_CANCELLED, *reason)); }
                    std::optional<google::protobuf::Duration> bound;
                    if (policy.has_timeout()) bound = policy.timeout() - (parent.now() - started);
                    if (policy.has_attempt_timeout() && (!bound || policy.attempt_timeout() < *bound)) bound = policy.attempt_timeout();
                    if (bound && *bound <= google::protobuf::Duration{}) throw Failure(interrupted(local, wire::CLASSIFICATION_TIMED_OUT, "Deadline Reached"));
                    std::optional<std::shared_future<std::string>> deadline;
                    if (bound) deadline = alarm(*bound);
                    auto done = std::make_shared<std::promise<std::string>>();
                    auto executing = done->get_future().share();
                    auto context = std::make_shared<Context>(local);
                    root->start_worker([root, context, done, operation, payload] {
                        try {
                            auto value = operation.handler(*context, payload);
                            std::lock_guard lock(root->mutex); done->set_value(std::move(value)); root->changed.notify_all();
                        } catch (...) {
                            std::lock_guard lock(root->mutex); done->set_exception(std::current_exception()); root->changed.notify_all();
                        }
                    });
                    auto ready = [](const std::shared_future<std::string>& future) { return future.wait_for(std::chrono::seconds(0)) == std::future_status::ready; };
                    std::optional<std::string> reason;
                    bool timed_out = false;
                    {
                        std::unique_lock lock(root->mutex);
                        root->changed.wait(lock, [&] { return ready(executing) || root->closed || cancellation(caller) || (deadline && ready(*deadline)); });
                        reason = cancellation(caller); timed_out = !reason && deadline && ready(*deadline);
                    }
                    if (!ready(executing)) {
                        if (timed_out) deadline->get();
                        const auto message = reason.value_or("Deadline Reached");
                        const auto kind = timed_out ? wire::CLASSIFICATION_TIMED_OUT : wire::CLASSIFICATION_CANCELLED;
                        cancel_owned(local, message, kind);
                        if (!timed_out && policy.has_cancellation_timeout() && policy.cancellation_timeout() > google::protobuf::Duration{}) {
                            auto settling = alarm(policy.cancellation_timeout());
                            std::unique_lock lock(root->mutex);
                            root->changed.wait(lock, [&] { return ready(executing) || root->closed || ready(settling) || (deadline && ready(*deadline)); });
                            timed_out = deadline && ready(*deadline);
                        }
                        if (timed_out || !policy.has_cancellation_timeout() || policy.cancellation_timeout() == google::protobuf::Duration{} || !ready(executing)) throw Failure(interrupted(local, kind, message));
                    }
                    auto value = executing.get();
                    cancel_owned(local, "Local Attempt Settled", wire::CLASSIFICATION_CANCELLED);
                    cancel_owned(guard, "Policy Guard Settled", wire::CLASSIFICATION_CANCELLED);
                    completion->set_value(std::move(value)); return;
                } catch (const Failure& failure) {
                    cancel_owned(local, "Local Attempt Settled", wire::CLASSIFICATION_CANCELLED);
                    cancel_owned(guard, "Policy Guard Settled", wire::CLASSIFICATION_CANCELLED);
                    if (failure.value.classification() != wire::CLASSIFICATION_TRANSIENT || attempt + 1 == attempts) throw;
                    auto delay = retry_delay(policy.retries(), attempt);
                    if (policy.has_timeout()) {
                        auto remaining = policy.timeout() - (parent.now() - started);
                        if (remaining < google::protobuf::Duration{}) remaining = google::protobuf::Duration{};
                        if (delay > remaining) delay = remaining;
                    }
                    if (delay > google::protobuf::Duration{}) parent.sleep(delay);
                } catch (...) {
                    cancel_owned(local, "Local Attempt Settled", wire::CLASSIFICATION_CANCELLED);
                    cancel_owned(guard, "Policy Guard Settled", wire::CLASSIFICATION_CANCELLED);
                    throw;
                }
            }
        } catch (...) { completion->set_exception(std::current_exception()); }
    });
    return result;
}

std::shared_future<std::string> Context::send(wire::ChildRequest request) {
    auto root = transport(state_);
    std::shared_future<std::string> result;
    {
        std::lock_guard lock(root->mutex);
        if (!root->emit) throw std::logic_error("Observers and Publishers Cannot Call Operations");
        if (root->closed) throw TransportError(14, "Invoke: Invocation Closed");
        if (!state_->background) if (auto reason = cancellation(state_)) throw Failure(interrupted(state_, wire::CLASSIFICATION_CANCELLED, *reason));
        auto found = state_->requirements.find(request.operation());
        if (found == state_->requirements.end() || found->second != request.fingerprint()) throw std::invalid_argument("Invoke " + request.operation() + ": Undeclared Requirement");
        request.set_call(++root->next); request.set_background(state_->background);
        auto& promise = root->pending[request.call()]; result = promise.get_future().share();
        state_->owned.insert(request.call()); if (state_->background) root->guards.insert(request.call());
    }
    plugin::v1::InstanceInvokeResponse frame; *frame.mutable_child() = std::move(request);
    root->emit(std::move(frame)); root->start_reading();
    return result;
}

std::string Context::cancelled() {
    auto root = transport(state_); root->start_reading();
    std::unique_lock lock(root->mutex);
    root->changed.wait(lock, [&] { return cancellation(state_).has_value() || root->closed; });
    return cancellation(state_).value_or("Invocation Closed");
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
