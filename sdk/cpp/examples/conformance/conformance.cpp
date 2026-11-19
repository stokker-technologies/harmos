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
#include "conformance.hpp"
#include "_wire/caller.hpp"
#include "_wire/contracts.hpp"
#include "generated/types.hpp"
#include "harmos/host/v1/invocations.pb.h"

using google::protobuf::Duration;
using std::optional;
using std::runtime_error;
using std::string;
using std::uint32_t;
using std::uint64_t;

/* === Macros === */

/* === Constants === */

/* === Types === */

using harmos::conformance::detail::lifecycle_operation;
using harmos::conformance::detail::steps_operation;

using ::conformance::model::v1::Values;
using harmos::conformance::Graph;
using harmos::conformance::Value;
using harmos::host::v1::InvocationsCreateRequest;
using harmos::host::v1::InvocationsCreateResponse;
using harmos::plugin::Context;

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

/* === Private Functions === */

namespace {
harmos::conformance::Value value(string bytes) {
  harmos::conformance::Value result;
  result.set_value(std::move(bytes));
  return result;
}

harmos::model::Failure failure() {
  harmos::model::Failure failure;
  failure.set_classification(harmos::CLASSIFICATION_PERMANENT);
  failure.set_code("fixture_failure");
  failure.set_message("Fixture Failed");
  failure.set_action("Use Another Input");
  failure.set_operation("conformance.failure");
  failure.set_plugin("conformance");
  (*failure.mutable_details())["fixture"] = "value";
  failure.set_diagnostics("Fixture Diagnostics");
  auto *cause = failure.mutable_cause();
  cause->set_classification(harmos::CLASSIFICATION_TRANSIENT);
  cause->set_code("fixture_cause");
  cause->set_message("Fixture Cause");
  cause->set_action("Retry");
  cause->set_operation("conformance.host.fail");
  cause->set_plugin("host");
  return failure;
}

/* === Entry Point === */

} // namespace

harmos::plugin::Plugin harmos::conformance::plugin() {
  harmos::plugin::Plugin result("conformance", "1.0.0");
  result.lane(harmos::plugin::Lane::serial("instrument"))
      .lane(harmos::plugin::Lane::concurrent("parallel", 2))
      .lane(harmos::plugin::Lane::unbounded("open"))
      .lane(harmos::plugin::Lane::serial("other"));
  result.add(catalog());
  return result;
}

std::function<void(harmos::plugin::Registrar &)> harmos::conformance::catalog() {
  return [](harmos::plugin::Registrar &registrar) {
    // Compose Typed Operations and Their Required Host Contracts.
    const harmos::plugin::Operation<Value, Value> echo("conformance.host.echo", harmos::VISIBILITY_PUBLIC);
    const harmos::plugin::Operation<Value, Value> pending("conformance.host.pending", harmos::VISIBILITY_PUBLIC);
    const harmos::plugin::Operation<Value, Value> fail("conformance.host.fail", harmos::VISIBILITY_PUBLIC);
    const auto instrument = harmos::plugin::Lane::serial("instrument");
    const auto parallel = harmos::plugin::Lane::concurrent("parallel", 2);
    const auto other = harmos::plugin::Lane::serial("other");
    for (const auto &name : {"first", "second", "parallel", "other", "open"}) {
      harmos::plugin::Operation<Value, Value> operation(string("conformance.lane.") + name, harmos::VISIBILITY_PUBLIC,
          [pending](Context &ctx, Value request) { return ctx.call(pending, request).get(); });
      if (string(name) == "first" || string(name) == "second") {
        operation.lane(instrument);
      } else if (string(name) == "parallel") {
        operation.lane(parallel);
      } else if (string(name) == "other") {
        operation.lane(other);
      }
      registrar.add(operation);
    }
    harmos::plugin::Operation<Value, Value> local("conformance.local", harmos::VISIBILITY_PRIVATE, [echo](Context &context, Value request) {
      if (request.value() == "deadline" || request.value() == "overall") {
        Duration delay;
        delay.set_seconds(3600);
        context.sleep(delay);
        return request;
      }
      if (request.value() == "cancel") {
        context.call(echo, request).get();
        return value(context.cancelled());
      }
      if (request.value() != "retry" && request.value() != "retry-bound")
        return request;
      if (context.attempt == 1) {
        auto error = failure();
        error.set_classification(harmos::CLASSIFICATION_TRANSIENT);
        throw harmos::Failure(error);
      }
      return value(std::to_string(context.attempt));
    });
    local.replay_safe();
    registrar.add(local).add(
        harmos::plugin::Operation<Value, Value>("conformance.local_call", harmos::VISIBILITY_PUBLIC, [local](Context &ctx, Value request) {
          optional<harmos::OperationPolicy> policy;
          if (request.value() == "deadline") {
            policy.emplace();
            policy->mutable_timeout()->set_seconds(3);
            policy->mutable_attempt_timeout()->set_seconds(2);
          }
          if (request.value() == "cancel") {
            policy.emplace();
            policy->mutable_cancellation_timeout()->set_seconds(1);
          }
          if (request.value() == "overall") {
            policy.emplace();
            policy->mutable_timeout()->set_seconds(2);
            policy->mutable_cancellation_timeout()->set_seconds(1);
          }
          if (request.value() == "retry-bound") {
            policy.emplace();
            policy->mutable_timeout()->set_seconds(1);
            policy->mutable_retries()->set_total_attempts(2);
            policy->mutable_retries()->mutable_initial()->set_seconds(3600);
          }
          if (request.value() == "retry") {
            policy.emplace();
            auto *retry = policy->mutable_retries();
            retry->set_total_attempts(2);
            retry->mutable_initial()->set_seconds(1);
            retry->mutable_maximum()->set_seconds(4);
            retry->set_deterministic_jitter(true);
          }
          return ctx.call(local, request, policy).get();
        }));
    registrar
        .add(harmos::plugin::Operation<Value, Value>("conformance.step.0", harmos::VISIBILITY_PUBLIC,
                                                     [](Context &ctx, Value request) {
                                                       return request.value() == "scope" ? value(ctx.scope()) : request;
                                                     })
                 .visibility(harmos::VISIBILITY_PUBLIC))
        .add(harmos::plugin::Operation<Value, Value>("conformance.children", harmos::VISIBILITY_PUBLIC,
                                                     [echo](Context &ctx, Value) {
                                                       auto first = ctx.call(echo, value(string(1, '\1')));
                                                       auto second = ctx.call(echo, value(string(1, '\2')));
                                                       return value(first.get().value() + second.get().value());
                                                     })
                 .visibility(harmos::VISIBILITY_PUBLIC))
        .add(harmos::plugin::Operation<Value, Value>("conformance.cancel", harmos::VISIBILITY_PUBLIC,
                                                     [pending](Context &ctx, Value request) {
                                                       return ctx.call(pending, request).get();
                                                     })
                 .visibility(harmos::VISIBILITY_PUBLIC))
        .add(harmos::plugin::Operation<Value, Value>("conformance.failure", harmos::VISIBILITY_PUBLIC,
                                                     [](Context &, Value) -> Value {
                                                       throw harmos::Failure(failure());
                                                     })
                 .visibility(harmos::VISIBILITY_PUBLIC))
        .add(harmos::plugin::Operation<Value, Value>("conformance.child_failure", harmos::VISIBILITY_PUBLIC,
                                                     [fail](Context &ctx, Value request) {
                                                       return ctx.call(fail, request).get();
                                                     })
                 .visibility(harmos::VISIBILITY_PUBLIC))
        .add(harmos::plugin::Dependency{echo})
        .add(harmos::plugin::Dependency{pending})
        .add(harmos::plugin::Dependency{fail});

    registrar.add(
        harmos::plugin::Dependency{harmos::plugin::Operation<Graph, Value>("conformance.host.schema", harmos::VISIBILITY_PUBLIC)});
    registrar.add(
        harmos::plugin::Operation<Value, Value>("conformance.policy", harmos::VISIBILITY_PUBLIC, [echo](Context &ctx, Value request) {
          harmos::OperationPolicy policy;
          policy.mutable_retries()->set_total_attempts(3);
          policy.mutable_retries()->mutable_initial()->set_seconds(1);
          policy.mutable_retries()->mutable_maximum()->set_seconds(4);
          policy.mutable_retries()->set_deterministic_jitter(true);
          const auto first = ctx.call(echo, request, policy).get();
          return ctx.call(echo, first).get();
        }).visibility(harmos::VISIBILITY_PUBLIC));
    registrar.add(
        harmos::plugin::Operation<Graph, Graph::Nested>("conformance.schema", harmos::VISIBILITY_PUBLIC, [](Context &, Graph request) {
          Graph::Nested response;
          response.set_count(request.items_size());
          *response.mutable_next() = std::move(request);
          return response;
        }).visibility(harmos::VISIBILITY_PUBLIC));
    registrar.add(harmos::plugin::Operation<Graph::Nested, Graph>("conformance.nested", harmos::VISIBILITY_PUBLIC,
                                                                  [](Context &, Graph::Nested request) {
                                                                    return request.next();
                                                                  })
                      .visibility(harmos::VISIBILITY_PUBLIC));

    registrar.add(harmos::plugin::Operation<Graph, Value>("conformance.projected", harmos::VISIBILITY_PUBLIC, [](Context &, Graph request) {
                    return request.tail().value();
                  }).visibility(harmos::VISIBILITY_PUBLIC));

    registrar.add(harmos::plugin::HostOperation::TimeSleep).add(harmos::plugin::HostOperation::TimeGet);
    registrar.add(harmos::plugin::HostOperation::PluginsSettingsList);
    registrar.add(harmos::plugin::HostOperation::OperationsList).add(harmos::plugin::HostOperation::OperationsGet);
    registrar.add(harmos::plugin::Operation<Value, Value>("conformance.clock", harmos::VISIBILITY_PUBLIC, [](Context &ctx, Value) {
                    const auto before = ctx.host().time.get();
                    Duration duration;
                    duration.set_seconds(3600);
                    ctx.host().time.sleep(duration);
                    const auto after = ctx.host().time.get();
                    const auto elapsed = static_cast<uint64_t>(after.seconds() - before.seconds());
                    string bytes(8, '\0');
                    for (unsigned index = 0; index < 8; ++index)
                      bytes[index] = static_cast<char>(elapsed >> (8 * index));
                    return value(bytes);
                  }).visibility(harmos::VISIBILITY_PUBLIC));

    registrar.add(harmos::plugin::Operation<Value, Value>(lifecycle_operation, harmos::VISIBILITY_PROTECTED, [](Context &, Value request) {
      return request;
    }));
    registrar.add(harmos::plugin::Operation<Value, Value>(steps_operation, harmos::VISIBILITY_PUBLIC,
                                                          [echo](Context &ctx, Value request) {
                                                            ctx.set(request);
                                                            ctx.call(echo, request).get();
                                                            return ctx.get<Value>();
                                                          })
                      .visibility(harmos::VISIBILITY_PUBLIC)
                      .replay_safe());
    registrar.add(harmos::plugin::Operation<Value, Value>("conformance.cancel_reason", harmos::VISIBILITY_PUBLIC,
                                                          [echo](Context &ctx, Value request) {
                                                            ctx.call(echo, request).get();
                                                            return value(ctx.cancelled());
                                                          })
                      .visibility(harmos::VISIBILITY_PUBLIC));

    // Compose a Host-Windowed Observer and a Device-Driven Stream.
    harmos::plugin::Stream<Value> interval_stream("conformance.interval");
    harmos::plugin::Observer<Value> observer(interval_stream, [](Context &ctx) {
      string bytes(8, '\0');
      for (unsigned index = 0; index < 8; ++index)
        bytes[index] = static_cast<char>(ctx.sequence >> (8 * index));
      return value(bytes);
    });
    harmos::plugin::Operation<Value, Value> host_watch("conformance.host.watch", harmos::VISIBILITY_PUBLIC);
    host_watch.watch();
    harmos::plugin::Operation<Value, Value> watch("conformance.watch", harmos::VISIBILITY_PUBLIC,
                                                  [host_watch](Context &ctx, Value request) {
                                                    auto watching = ctx.watch(host_watch, request, 4096);
                                                    string result;
                                                    while (auto item = watching.next()) {
                                                      result += item->value();
                                                      if (request.value() == "cancel")
                                                        break;
                                                    }
                                                    return value(result);
                                                  });
    registrar.add(watch).add(harmos::plugin::Dependency{host_watch});
    const harmos::plugin::Operation<Value, Value> peer_public("conformance.peer.public", harmos::VISIBILITY_PUBLIC);
    const harmos::plugin::Operation<Value, Value> peer_protected("conformance.peer.protected", harmos::VISIBILITY_PROTECTED);
    const harmos::plugin::Operation<Value, Value> peer_local("conformance.peer.local", harmos::VISIBILITY_PROTECTED);
    const harmos::plugin::Operation<Value, Value> own_protected(lifecycle_operation, harmos::VISIBILITY_PROTECTED);
    registrar
        .add(harmos::plugin::Operation<Value, Value>(
            "conformance.peer_call", harmos::VISIBILITY_PUBLIC,
            [peer_public, peer_protected, peer_local, own_protected](Context &context, Value request) {
              if (request.value() == "self")
                return context.call(own_protected, request).get();
              if (request.value() == "public")
                return context.call(peer_public, value(context.scope())).get();
              return context.call(request.value() == "protected" ? peer_protected : peer_local, request).get();
            }))
        .add(harmos::plugin::Dependency{peer_public})
        .add(harmos::plugin::Dependency{peer_protected})
        .add(harmos::plugin::Dependency{peer_local})
        .add(harmos::plugin::Dependency{own_protected});
    registrar.add(
        harmos::plugin::Operation<Value, Value>("conformance.settings_read", harmos::VISIBILITY_PUBLIC, [](Context &context, Value) {
          return value(harmos::plugin::detail::encode(context.host().plugins.settings.list().value()));
        }));
    registrar.add(harmos::plugin::Operation<Value, Value>("conformance.discover", harmos::VISIBILITY_PUBLIC, [](Context &context, Value) {
      const auto listed = context.host().operations.list();
      if (listed.empty())
        throw runtime_error("List Operations: Missing Contract");
      const auto &operation = listed.at(0);
      const auto found = context.host().operations.get(operation.contract().id());
      if (harmos::plugin::detail::encode(found) != harmos::plugin::detail::encode(operation))
        throw runtime_error("Get Operation: Contract Differs");
      return value(operation.contract().id());
    }));

    Duration interval;
    interval.set_seconds(1);
    observer.every(interval);
    harmos::plugin::Stream<Value> device("conformance.device");
    device.publish_on([](Context &ctx, harmos::plugin::Publisher<Value> publisher) {
      publisher.publish(value("device"));
      ctx.cancelled();
    });
    harmos::plugin::Stream<Value> settings("conformance.settings");
    harmos::plugin::Observer<Value> configured(settings, [](Context &ctx) {
      string bytes(8, '\0');
      for (unsigned index = 0; index < 8; ++index)
        bytes[index] = static_cast<char>(ctx.sequence >> (8 * index));
      return value(bytes);
    });
    configured.every(interval).interval_setting("interval");
    Graph section;
    section.mutable_interval()->set_seconds(2);
    registrar.add(harmos::plugin::Descriptor<harmos::conformance::PayloadOnly>{})
        .add(harmos::plugin::Descriptor<harmos::conformance::PayloadOnly>{});
    registrar.add(interval_stream).add(device).add(settings).add(observer).add(configured).add(harmos::plugin::Contribution{section});

    for (const auto *name : {"count", "enabled", "gain", "interval", "label"}) {
      harmos::SettingDeclaration setting;
      setting.set_name(name);
      setting.set_title(name);
      setting.set_description(string("Conformance ") + name);
      const string key(name);
      if (key == "count")
        setting.mutable_default_()->set_integer(3);
      if (key == "enabled")
        setting.mutable_default_()->set_flag(false);
      if (key == "gain")
        setting.mutable_default_()->set_number(1.5);
      if (key == "interval")
        setting.mutable_default_()->mutable_duration()->set_seconds(2);
      if (key == "label")
        setting.mutable_default_()->set_text("default");
      registrar.add(std::move(setting));
    }

    harmos::plugin::Stream<Value> children("conformance.window_children");
    harmos::plugin::Observer<Value> child_observer(children, [echo](Context &context) {
      auto first = context.call(echo, value(string(1, '\1')));
      auto second = context.call(echo, value(string(1, '\2')));
      return value(first.get().value() + second.get().value());
    });
    registrar.add(children).add(child_observer.every(interval));

    harmos::plugin::Stream<Value> calls("conformance.window_context");
    harmos::plugin::Observer<Value> call_observer(calls, [echo](Context &context) {
      const auto echoed = context.call(echo, value(context.scope())).get();
      harmos::plugin::TimeGetResponse time;
      *time.mutable_now() = context.host().time.get();
      const auto setting_values = context.host().plugins.settings.list();
      string bytes;
      for (const auto &reply :
           {harmos::plugin::detail::encode(echoed), harmos::plugin::detail::encode(time), harmos::plugin::detail::encode(setting_values)}) {
        const auto length = static_cast<uint32_t>(reply.size());
        for (unsigned index = 0; index < 4; ++index)
          bytes += static_cast<char>(length >> (8 * index));
        bytes += reply;
      }
      return value(bytes);
    });
    registrar.add(calls).add(call_observer.every(interval));

    harmos::plugin::Stream<Value> failures("conformance.window_failure");
    harmos::plugin::Observer<Value> failure_observer(failures, [fail](Context &context) {
      try {
        context.call(fail, Value{}).get();
      } catch (const harmos::Failure &failure) {
        return value(harmos::plugin::detail::encode(failure.value));
      }
      throw runtime_error("Sample Failure: Expected Child Failure");
    });
    registrar.add(failures).add(failure_observer.every(interval));

    harmos::plugin::Stream<Value> watches("conformance.window_watch");
    harmos::plugin::Observer<Value> watch_observer(watches, [host_watch](Context &context) {
      auto watching = context.watch(host_watch, value(context.scope()), 4096);
      string result;
      while (auto item = watching.next())
        result += item->value();
      return value(result);
    });
    registrar.add(watches).add(watch_observer.every(interval));

    harmos::plugin::Stream<Value> cancellations("conformance.window_cancel");
    harmos::plugin::Observer<Value> cancel_observer(cancellations, [pending, echo](Context &context) {
      auto child = context.call(pending, Value{});
      context.call(echo, value("release")).get();
      child.cancel();
      context.call(pending, Value{});
      return value("cancelled");
    });
    registrar.add(cancellations).add(cancel_observer.every(interval));

    const harmos::plugin::Operation<InvocationsCreateRequest, InvocationsCreateResponse> create("application.host.v1.Invocations/Create",
                                                                                                harmos::VISIBILITY_PUBLIC);
    const auto refuse = [create](Context &context, Value input) {
      InvocationsCreateRequest request;
      if (!request.ParseFromArray(harmos::conformance::caller_request.data(),
                                  static_cast<int>(harmos::conformance::caller_request.size()))) {
        throw runtime_error("Create Invocation: Invalid Generated Probe Request");
      }
      try {
        context.call(create, request).get();
      } catch (const harmos::Failure &failure) {
        if (input.value() == "propagate")
          throw;
        return value(harmos::plugin::detail::encode(failure.value));
      }
      throw runtime_error("Create Invocation: Client-Only Operation Was Admitted");
    };
    registrar.add(harmos::plugin::Operation<Value, Value>("conformance.caller_refusal", harmos::VISIBILITY_PUBLIC, refuse))
        .add(harmos::plugin::Dependency{create});
    harmos::plugin::Stream<Value> refusals("conformance.window_caller_refusal");
    harmos::plugin::Observer<Value> refusal_observer(refusals, [refuse](Context &context) {
      return refuse(context, Value{});
    });
    registrar.add(refusals).add(refusal_observer.every(interval));

    // Answer a Whole Window at Once: Each Value Holds Its Sample's Sequence.
    harmos::plugin::Stream<Value> window("conformance.window");
    auto columns = harmos::plugin::Observer<Value>::window<Values>(window, [](Context &, const harmos::plugin::Window &grant) {
      Values result;
      for (uint64_t index = 0; index < grant.count; ++index) {
        const auto sequence = grant.sequence + index;
        string bytes(8, '\0');
        for (unsigned position = 0; position < 8; ++position)
          bytes[position] = static_cast<char>(sequence >> (8 * position));
        result.add_value(std::move(bytes));
      }
      return result;
    });
    registrar.add(window).add(columns.every(interval));

    // Each Sample Holds as Many Bytes as Its Sequence, So a Grant Can Exceed the Message Size.
    harmos::plugin::Stream<Value> oversized("conformance.oversized");
    harmos::plugin::Observer<Value> oversized_observer(oversized, [](Context &ctx) {
      Value result;
      result.set_value(string(static_cast<size_t>(ctx.sequence), static_cast<char>(0)));
      return result;
    });
    registrar.add(oversized).add(oversized_observer.every(interval));
  };
}
