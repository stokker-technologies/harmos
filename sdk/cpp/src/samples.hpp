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
#include <cstddef>
#include <limits>

// Third Party Imports

// Local Imports
#include "columns.hpp"
#include "duplex.hpp"
#include "invocation.hpp"

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

namespace harmos::plugin::detail {
class PublicationCall final : public Duplex<plugin::v1::InstanceSamplesRequest, plugin::v1::InstanceSamplesResponse> {
public:
  // `message_bytes` Bounds Every Encoded Response, So Window Answers Split to Fit It.
  PublicationCall(const std::vector<StreamDefinition> &streams, const std::map<std::string, std::string> &requirements,
                  const std::string &plugin, std::size_t message_bytes)
      : streams_(streams), state_(std::make_shared<Invocation>()), message_bytes_(message_bytes) {
    state_->window_scoped = true;
    state_->requirements = requirements;
    state_->plugin = plugin;
    state_->emit = [this](plugin::v1::InstanceInvokeResponse frame) {
      std::lock_guard lock(window_mutex_);
      if (!state_->window_active)
        throw TransportError(3, "Send Samples Child: Call Children Only While Answering a Window");
      if (!state_->accepting_children && !frame.has_child_cancel())
        throw TransportError(3, "Send Samples Child: Call Children Before the Window Is Answered");
      plugin::v1::InstanceSamplesResponse samples;
      if (frame.has_child())
        *samples.mutable_child() = std::move(*frame.mutable_child());
      else if (frame.has_child_demand())
        *samples.mutable_child_demand() = frame.child_demand();
      else if (frame.has_child_cancel())
        *samples.mutable_child_cancel() = frame.child_cancel();
      else
        throw TransportError(3, "Send Samples Child: Send a Child Request, Demand or Cancel");
      write(std::move(samples));
    };
    read_next();
  }

  void settle_children() override {
    state_->settle_workers();
    if (sampling_) {
      sampling_ = false;
      Context context(state_);
      try {
        stream_->sampling->change(context, false);
      } catch (const std::exception &error) {
        end({grpc::StatusCode::INTERNAL, error.what()});
      }
    }
  }
  void closed() override {
    state_->close(std::make_exception_ptr(TransportError(1, "Sample: Demand Closed")));
  }

  void OnReadDone(bool ok) override {
    if (!ok) {
      end(stream_ ? grpc::Status::OK : grpc::Status(grpc::StatusCode::INVALID_ARGUMENT, "Sample: Open Must Be First"));
      closed();
      return;
    }
    if (!stream_) {
      if (!input.has_open()) {
        refuse("Open Must Be First");
        return;
      }
      for (const auto &stream : streams_)
        if (stream.declaration.id() == input.open().stream())
          stream_ = &stream;
      if (!stream_) {
        end({grpc::StatusCode::NOT_FOUND, "Sample: Unknown Stream"});
        return;
      }
      state_->request.set_scope(input.open().scope());
      state_->request.set_operation(input.open().stream());
      state_->request.set_attempt(1);
      if (stream_->producer) {
        worker = std::thread([this] {
          try {
            Context context(state_);
            stream_->producer(context, [this](std::string row, std::optional<google::protobuf::Timestamp> at) {
              if (at && !valid_timestamp(*at))
                throw TransportError(3, "Publish Sample: Invalid Device Timestamp");
              plugin::v1::InstanceSamplesResponse samples;
              auto *sample = samples.add_samples();
              sample->set_row(std::move(row));
              sample->set_sequence(++sequence_);
              if (at)
                *sample->mutable_at() = std::move(*at);
              write(std::move(samples));
            });
          } catch (const TransportError &error) {
            end({static_cast<grpc::StatusCode>(error.code), error.what()});
          } catch (const std::exception &error) {
            end({grpc::StatusCode::INTERNAL, error.what()});
          }
        });
      }
      read_next();
      return;
    }
    if (input.has_wind_down() && input.wind_down()) {
      state_->wind_down_requested = true;
      read_next();
      return;
    }
    if (input.has_demand()) {
      const auto &selected = input.demand().streams();
      if (stream_->producer || selected.size() > 1 || (selected.size() == 1 && selected[0] != stream_->declaration.id())) {
        refuse("Select This Observer's Stream Once or Release It");
        return;
      }
      {
        std::lock_guard lock(state_->mutex);
        if (state_->window_active) {
          refuse("Change Sampling Demand After the Window Is Answered");
          return;
        }
      }
      if (worker.joinable())
        worker.join();
      const bool wanted = !selected.empty();
      worker = std::thread([this, wanted] {
        try {
          if (sampling_ != wanted) {
            if (!wanted)
              sampling_ = false;
            Context context(state_);
            stream_->sampling->change(context, wanted);
            sampling_ = wanted;
          }
          plugin::v1::InstanceSamplesResponse response;
          response.mutable_demand();
          write(std::move(response));
          read_next();
        } catch (const std::exception &error) {
          end({grpc::StatusCode::INTERNAL, error.what()});
          closed();
        }
      });
      return;
    }
    if (input.has_child()) {
      // Retired IDs Belong Only to the Current Window's Cancelled Children.
      if (!state_->receive(input.child())) {
        refuse("Reply Only to Open Children of the Unanswered Window");
        return;
      }
      read_next();
      return;
    }
    if (!input.has_window()) {
      refuse("Send Only Window or Child Frames After Open");
      return;
    }
    const auto &grant = input.window();
    if (!stream_->sample && !stream_->window) {
      refuse("Send Windows Only to Interval Streams");
      return;
    }
    if (!grant.has_start() || !valid_timestamp(grant.start())) {
      refuse("Send a Window with a Valid Start Timestamp");
      return;
    }
    if (!grant.has_period() || !positive(grant.period())) {
      refuse("Send a Window with a Positive Period");
      return;
    }
    if (grant.count() == 0 || grant.sequence() > std::numeric_limits<std::uint64_t>::max() - (grant.count() - 1)) {
      refuse("Send a Window with at Least One Sample and a Representable Sequence");
      return;
    }
    bool active;
    {
      std::lock_guard lock(state_->mutex);
      active = state_->window_active;
      if (!active) {
        state_->window_active = true;
        state_->accepting_children = true;
        state_->retired.clear();
      }
    }
    if (active) {
      refuse("Await the Last Part of the Unanswered Window");
      return;
    }
    if (worker.joinable())
      worker.join();
    auto window_state = std::make_shared<Invocation>();
    window_state->transport = state_;
    window_state->request = state_->request;
    *window_state->request.mutable_capturing_executions() = grant.capturing_executions();
    window_state->requirements = state_->requirements;
    window_state->plugin = state_->plugin;
    const Window window{grant.start(), grant.period(), grant.count(), grant.sequence()};
    worker = std::thread([this, window_state, window] {
      try {
        answer(window_state, window);
      } catch (const TransportError &error) {
        end({static_cast<grpc::StatusCode>(error.code), error.what()});
        closed();
      } catch (const std::exception &error) {
        end({grpc::StatusCode::INTERNAL, error.what()});
        closed();
      }
    });
    read_next();
  }

private:
  const std::vector<StreamDefinition> &streams_;
  const StreamDefinition *stream_ = nullptr;
  std::shared_ptr<Invocation> state_;
  std::size_t message_bytes_;
  std::recursive_mutex window_mutex_;
  std::uint64_t sequence_ = 0;
  bool sampling_ = false;

  void answer(const std::shared_ptr<Invocation> &window_state, const Window &window) {
    // Run the Observer, Then Close the Window's Children Before Any Part Is Sent.
    Columns columns(stream_->row);
    if (stream_->window) {
      Context context(window_state);
      context.sequence = window.sequence;
      columns.extend(*stream_->window(context, window), window.count);
    } else {
      for (std::uint64_t index = 0; index < window.count; ++index) {
        Context context(window_state);
        context.sequence = window.sequence + index;
        columns.append(*stream_->sample(context));
      }
    }
    state_->accepting_children = false;
    cancel_children(window_state, "Samples Window Answered");
    const auto parts = columns.parts(message_bytes_);
    for (const auto &[begin, end] : parts) {
      plugin::v1::InstanceSamplesResponse response;
      auto *batch = response.mutable_batch();
      batch->set_offset(begin);
      batch->set_count(end - begin);
      batch->set_columns(columns.encode(begin, end));
      batch->set_last(end == window.count);
      if (!batch->last()) {
        write(std::move(response));
        continue;
      }
      // The Last Part Answers the Window; the Host May Grant the Next One Once It Arrives.
      std::lock_guard lock(window_mutex_);
      {
        std::lock_guard state_lock(state_->mutex);
        state_->window_active = false;
      }
      write(std::move(response));
    }
  }
  static bool valid_timestamp(const google::protobuf::Timestamp &at) {
    return at.seconds() >= -62135596800LL && at.seconds() <= 253402300799LL && at.nanos() >= 0 && at.nanos() < 1000000000;
  }
  static bool positive(const google::protobuf::Duration &period) {
    const bool valid = period.seconds() >= 0 && period.seconds() <= 315576000000LL && period.nanos() >= 0 && period.nanos() < 1000000000;
    return valid && (period.seconds() > 0 || period.nanos() > 0);
  }
  void refuse(const std::string &reason) {
    end({grpc::StatusCode::INVALID_ARGUMENT, "Sample: " + reason});
    closed();
  }
};
} // namespace harmos::plugin::detail

/* === Templates === */

/* === Public Declarations === */
