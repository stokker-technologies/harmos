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

// Third Party Imports

// Local Imports
#include "duplex.hpp"
#include "invocation.hpp"

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

namespace harmos::detail {
class PublicationCall final : public Duplex<plugin::v1::InstanceSamplesRequest, plugin::v1::InstanceSamplesResponse> {
public:
    PublicationCall(const std::vector<StreamDefinition>& streams, const std::vector<OperationDefinition>& operations,
                    const std::map<std::string, std::string>& requirements, const std::string& plugin)
        : streams_(streams), state_(std::make_shared<Invocation>()) {
        state_->tick_scoped = true;
        state_->requirements = requirements;
        state_->plugin = plugin;
        for (const auto& operation : operations) if (operation.contract.visibility() == wire::VISIBILITY_PRIVATE)
            state_->local.emplace(operation.contract.id(), operation);
        state_->emit = [this](plugin::v1::InstanceInvokeResponse frame) {
            std::lock_guard lock(tick_mutex_);
            if (!state_->tick_active) throw TransportError(3, "Send Samples Child: An Unanswered Tick Is Required");
            if (!state_->accepting_children && !frame.has_child_cancel()) throw TransportError(3, "Send Samples Child: Tick Is Answered");
            plugin::v1::InstanceSamplesResponse samples;
            if (frame.has_child()) *samples.mutable_child() = std::move(*frame.mutable_child());
            else if (frame.has_child_demand()) *samples.mutable_child_demand() = frame.child_demand();
            else if (frame.has_child_cancel()) *samples.mutable_child_cancel() = frame.child_cancel();
            else throw TransportError(3, "Send Samples Child: Invalid Child Frame");
            write(std::move(samples));
        };
        read_next();
    }

    void settle_children() override { state_->settle_workers(); }
    void closed() override { state_->close(std::make_exception_ptr(TransportError(1, "Sample: Demand Closed"))); }

    void OnReadDone(bool ok) override {
        if (!ok) {
            end(stream_ ? grpc::Status::OK : grpc::Status(grpc::StatusCode::INVALID_ARGUMENT, "Sample: Open Must Be First"));
            closed();
            return;
        }
        if (!stream_) {
            if (!input.has_open()) { refuse("Open Must Be First"); return; }
            for (const auto& stream : streams_) if (stream.declaration.id() == input.open().stream()) stream_ = &stream;
            if (!stream_) { end({grpc::StatusCode::NOT_FOUND, "Sample: Unknown Stream"}); return; }
            state_->request.set_scope(input.open().scope());
            state_->request.set_operation(input.open().stream());
            state_->request.set_attempt(1);
            if (stream_->producer) {
                worker = std::thread([this] {
                    try {
                        Context context(state_);
                        stream_->producer(context, [this](std::string row, std::optional<google::protobuf::Timestamp> at) {
                            if (at && !valid_timestamp(*at)) throw TransportError(3, "Publish Sample: Invalid Device Timestamp");
                            plugin::v1::InstanceSamplesResponse samples;
                            auto* sample = samples.add_samples();
                            sample->set_row(std::move(row));
                            sample->set_sequence(++sequence_);
                            if (at) *sample->mutable_at() = std::move(*at);
                            write(std::move(samples));
                        });
                    } catch (const TransportError& error) {
                        end({static_cast<grpc::StatusCode>(error.code), error.what()});
                    } catch (const std::exception& error) { end({grpc::StatusCode::INTERNAL, error.what()}); }
                });
            }
            read_next();
            return;
        }
        if (input.has_child()) {
            // Retired IDs Belong Only to the Current Tick's Explicit Cancellations.
            if (!state_->receive(input.child())) { refuse("Unknown, Duplicate or Outside-Tick Child Reply"); return; }
            read_next();
            return;
        }
        if (!input.has_tick() || !input.tick().has_at() || !valid_timestamp(input.tick().at()) || !stream_->observer) {
            refuse("Expected a Valid Observer Tick");
            return;
        }
        bool active;
        {
            std::lock_guard lock(state_->mutex);
            active = state_->tick_active;
            if (!active) {
                state_->tick_active = true;
                state_->accepting_children = true;
                state_->retired.clear();
            }
        }
        if (active) { refuse("Previous Tick Is Unanswered"); return; }
        if (worker.joinable()) worker.join();
        auto tick = std::make_shared<Invocation>();
        tick->transport = state_;
        tick->request = state_->request;
        tick->requirements = state_->requirements;
        tick->local = state_->local;
        tick->plugin = state_->plugin;
        const auto at = input.tick().at();
        const auto sequence = ++sequence_;
        worker = std::thread([this, tick, at, sequence] {
            try {
                Context context(tick);
                context.sequence = sequence;
                const auto row = stream_->observer(context);
                // Cancel Open Children Before Answering and Retire This Tick's Context.
                state_->accepting_children = false;
                cancel_children(tick, "Sample: Tick Answered");
                plugin::v1::InstanceSamplesResponse samples;
                auto* sample = samples.add_samples();
                *sample->mutable_at() = at;
                sample->set_sequence(sequence);
                sample->set_row(row);
                std::lock_guard lock(tick_mutex_);
                {
                    std::lock_guard state_lock(state_->mutex);
                    state_->tick_active = false;
                }
                write(std::move(samples));
            } catch (const TransportError& error) {
                end({static_cast<grpc::StatusCode>(error.code), error.what()});
                closed();
            } catch (const std::exception& error) {
                end({grpc::StatusCode::INTERNAL, error.what()});
                closed();
            }
        });
        read_next();
    }

private:
    const std::vector<StreamDefinition>& streams_;
    const StreamDefinition* stream_ = nullptr;
    std::shared_ptr<Invocation> state_;
    std::recursive_mutex tick_mutex_;
    std::uint64_t sequence_ = 0;

    static bool valid_timestamp(const google::protobuf::Timestamp& at) {
        return at.seconds() >= -62135596800LL && at.seconds() <= 253402300799LL && at.nanos() >= 0 && at.nanos() < 1000000000;
    }
    void refuse(const std::string& reason) {
        end({grpc::StatusCode::INVALID_ARGUMENT, "Sample: " + reason});
        closed();
    }
};
}

/* === Templates === */

/* === Public Declarations === */
