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

harmos::FailureValue failure() {
  harmos::FailureValue failure;
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
  // Compose Typed Operations and Their Required Host Contracts.
  const harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value> echo("conformance.host.echo",
                                                                                               harmos::VISIBILITY_PUBLIC);
  const harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value> pending("conformance.host.pending",
                                                                                                  harmos::VISIBILITY_PUBLIC);
  const harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value> fail("conformance.host.fail",
                                                                                               harmos::VISIBILITY_PUBLIC);
  harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value> local(
      "conformance.local", harmos::VISIBILITY_PRIVATE, [echo](Context &context, harmos::conformance::Value request) {
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
  harmos::plugin::Plugin plugin("conformance", "1.0.0");
  plugin.operation(local).operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
      "conformance.local_call", harmos::VISIBILITY_PUBLIC, [local](Context &ctx, harmos::conformance::Value request) {
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
  plugin
      .operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
                     "conformance.step.0", harmos::VISIBILITY_PUBLIC,
                     [](Context &ctx, harmos::conformance::Value request) {
                       return request.value() == "scope" ? value(ctx.scope()) : request;
                     })
                     .visibility(harmos::VISIBILITY_PUBLIC))
      .operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
                     "conformance.children", harmos::VISIBILITY_PUBLIC,
                     [echo](Context &ctx, harmos::conformance::Value) {
                       auto first = ctx.call(echo, value(string(1, '\1')));
                       auto second = ctx.call(echo, value(string(1, '\2')));
                       return value(first.get().value() + second.get().value());
                     })
                     .visibility(harmos::VISIBILITY_PUBLIC))
      .operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
                     "conformance.cancel", harmos::VISIBILITY_PUBLIC,
                     [pending](Context &ctx, harmos::conformance::Value request) {
                       return ctx.call(pending, request).get();
                     })
                     .visibility(harmos::VISIBILITY_PUBLIC))
      .operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
                     "conformance.failure", harmos::VISIBILITY_PUBLIC,
                     [](Context &, harmos::conformance::Value) -> harmos::conformance::Value {
                       throw harmos::Failure(failure());
                     })
                     .visibility(harmos::VISIBILITY_PUBLIC))
      .operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
                     "conformance.child_failure", harmos::VISIBILITY_PUBLIC,
                     [fail](Context &ctx, harmos::conformance::Value request) {
                       return ctx.call(fail, request).get();
                     })
                     .visibility(harmos::VISIBILITY_PUBLIC))
      .requires_operation(echo)
      .requires_operation(pending)
      .requires_operation(fail);

  plugin.requires_operation(harmos::plugin::Operation<harmos::conformance::Graph, harmos::conformance::Value>("conformance.host.schema",
                                                                                                              harmos::VISIBILITY_PUBLIC));
  plugin.operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
                       "conformance.policy", harmos::VISIBILITY_PUBLIC,
                       [echo](Context &ctx, harmos::conformance::Value request) {
                         harmos::OperationPolicy policy;
                         policy.mutable_retries()->set_total_attempts(3);
                         policy.mutable_retries()->mutable_initial()->set_seconds(1);
                         policy.mutable_retries()->mutable_maximum()->set_seconds(4);
                         policy.mutable_retries()->set_deterministic_jitter(true);
                         const auto first = ctx.call(echo, request, policy).get();
                         return ctx.call(echo, first).get();
                       })
                       .visibility(harmos::VISIBILITY_PUBLIC));
  plugin.operation(harmos::plugin::Operation<harmos::conformance::Graph, harmos::conformance::Graph::Nested>(
                       "conformance.schema", harmos::VISIBILITY_PUBLIC,
                       [](Context &, harmos::conformance::Graph request) {
                         harmos::conformance::Graph::Nested response;
                         response.set_count(request.items_size());
                         *response.mutable_next() = std::move(request);
                         return response;
                       })
                       .visibility(harmos::VISIBILITY_PUBLIC));
  plugin.operation(harmos::plugin::Operation<harmos::conformance::Graph::Nested, harmos::conformance::Graph>(
                       "conformance.nested", harmos::VISIBILITY_PUBLIC,
                       [](Context &, harmos::conformance::Graph::Nested request) {
                         return request.next();
                       })
                       .visibility(harmos::VISIBILITY_PUBLIC));

  plugin.operation(
      harmos::plugin::Operation<harmos::conformance::Graph, harmos::conformance::Value>("conformance.projected", harmos::VISIBILITY_PUBLIC,
                                                                                        [](Context &, harmos::conformance::Graph request) {
                                                                                          return request.tail().value();
                                                                                        })
          .visibility(harmos::VISIBILITY_PUBLIC));

  plugin.requires_operation(harmos::plugin::HostOperation::TimeSleep).requires_operation(harmos::plugin::HostOperation::TimeGet);
  plugin.requires_operation(harmos::plugin::HostOperation::PluginsSettingsList);
  plugin.requires_operation(harmos::plugin::HostOperation::OperationsList).requires_operation(harmos::plugin::HostOperation::OperationsGet);
  plugin.operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
                       "conformance.clock", harmos::VISIBILITY_PUBLIC,
                       [](Context &ctx, harmos::conformance::Value) {
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
                       })
                       .visibility(harmos::VISIBILITY_PUBLIC));

  plugin.operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
      lifecycle_operation, harmos::VISIBILITY_PROTECTED, [](Context &, harmos::conformance::Value request) {
        return request;
      }));
  plugin.operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
                       steps_operation, harmos::VISIBILITY_PUBLIC,
                       [echo](Context &ctx, harmos::conformance::Value request) {
                         ctx.set(request);
                         ctx.call(echo, request).get();
                         return ctx.get<harmos::conformance::Value>();
                       })
                       .visibility(harmos::VISIBILITY_PUBLIC)
                       .replay_safe());
  plugin.operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
                       "conformance.cancel_reason", harmos::VISIBILITY_PUBLIC,
                       [echo](Context &ctx, harmos::conformance::Value request) {
                         ctx.call(echo, request).get();
                         return value(ctx.cancelled());
                       })
                       .visibility(harmos::VISIBILITY_PUBLIC));

  // Compose a Host-Windowed Observer and a Device-Driven Stream.
  harmos::plugin::Stream<harmos::conformance::Value> interval_stream("conformance.interval");
  harmos::plugin::Observer<harmos::conformance::Value> observer(interval_stream, [](Context &ctx) {
    string bytes(8, '\0');
    for (unsigned index = 0; index < 8; ++index)
      bytes[index] = static_cast<char>(ctx.sequence >> (8 * index));
    return value(bytes);
  });
  harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value> host_watch("conformance.host.watch",
                                                                                               harmos::VISIBILITY_PUBLIC);
  host_watch.watch();
  harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value> watch(
      "conformance.watch", harmos::VISIBILITY_PUBLIC, [host_watch](Context &ctx, harmos::conformance::Value request) {
        auto watching = ctx.watch(host_watch, request, 4096);
        string result;
        while (auto item = watching.next()) {
          result += item->value();
          if (request.value() == "cancel")
            break;
        }
        return value(result);
      });
  plugin.operation(watch).requires_operation(host_watch);
  const harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value> peer_public("conformance.peer.public",
                                                                                                      harmos::VISIBILITY_PUBLIC);
  const harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value> peer_protected("conformance.peer.protected",
                                                                                                         harmos::VISIBILITY_PROTECTED);
  const harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value> peer_local("conformance.peer.local",
                                                                                                     harmos::VISIBILITY_PROTECTED);
  const harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value> own_protected(lifecycle_operation,
                                                                                                        harmos::VISIBILITY_PROTECTED);
  plugin
      .operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
          "conformance.peer_call", harmos::VISIBILITY_PUBLIC,
          [peer_public, peer_protected, peer_local, own_protected](Context &context, harmos::conformance::Value request) {
            if (request.value() == "self")
              return context.call(own_protected, request).get();
            if (request.value() == "public")
              return context.call(peer_public, value(context.scope())).get();
            return context.call(request.value() == "protected" ? peer_protected : peer_local, request).get();
          }))
      .requires_operation(peer_public)
      .requires_operation(peer_protected)
      .requires_operation(peer_local)
      .requires_operation(own_protected);
  plugin.operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
      "conformance.settings_read", harmos::VISIBILITY_PUBLIC, [](Context &context, harmos::conformance::Value) {
        return value(harmos::plugin::detail::encode(context.host().plugins.settings.list().value()));
      }));
  plugin.operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>(
      "conformance.discover", harmos::VISIBILITY_PUBLIC, [](Context &context, harmos::conformance::Value) {
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
  harmos::plugin::Stream<harmos::conformance::Value> device("conformance.device");
  device.publish_on([](Context &ctx, harmos::plugin::Publisher<harmos::conformance::Value> publisher) {
    publisher.publish(value("device"));
    ctx.cancelled();
  });
  harmos::plugin::Stream<harmos::conformance::Value> settings("conformance.settings");
  harmos::plugin::Observer<harmos::conformance::Value> configured(settings, [](Context &ctx) {
    string bytes(8, '\0');
    for (unsigned index = 0; index < 8; ++index)
      bytes[index] = static_cast<char>(ctx.sequence >> (8 * index));
    return value(bytes);
  });
  configured.every(interval).interval_setting("interval");
  harmos::conformance::Graph section;
  section.mutable_interval()->set_seconds(2);
  plugin.message<harmos::conformance::PayloadOnly>().message<harmos::conformance::PayloadOnly>();
  plugin.stream(interval_stream).stream(device).stream(settings).observer(observer).observer(configured).section(section);

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
    plugin.setting(std::move(setting));
  }

  harmos::plugin::Stream<harmos::conformance::Value> children("conformance.window_children");
  harmos::plugin::Observer<harmos::conformance::Value> child_observer(children, [echo](Context &context) {
    auto first = context.call(echo, value(string(1, '\1')));
    auto second = context.call(echo, value(string(1, '\2')));
    return value(first.get().value() + second.get().value());
  });
  plugin.stream(children).observer(child_observer.every(interval));

  harmos::plugin::Stream<harmos::conformance::Value> calls("conformance.window_context");
  harmos::plugin::Observer<harmos::conformance::Value> call_observer(calls, [echo](Context &context) {
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
  plugin.stream(calls).observer(call_observer.every(interval));

  harmos::plugin::Stream<harmos::conformance::Value> failures("conformance.window_failure");
  harmos::plugin::Observer<harmos::conformance::Value> failure_observer(failures, [fail](Context &context) {
    try {
      context.call(fail, harmos::conformance::Value{}).get();
    } catch (const harmos::Failure &failure) {
      return value(harmos::plugin::detail::encode(failure.value));
    }
    throw runtime_error("Sample Failure: Expected Child Failure");
  });
  plugin.stream(failures).observer(failure_observer.every(interval));

  harmos::plugin::Stream<harmos::conformance::Value> watches("conformance.window_watch");
  harmos::plugin::Observer<harmos::conformance::Value> watch_observer(watches, [host_watch](Context &context) {
    auto watching = context.watch(host_watch, value(context.scope()), 4096);
    string result;
    while (auto item = watching.next())
      result += item->value();
    return value(result);
  });
  plugin.stream(watches).observer(watch_observer.every(interval));

  harmos::plugin::Stream<harmos::conformance::Value> cancellations("conformance.window_cancel");
  harmos::plugin::Observer<harmos::conformance::Value> cancel_observer(cancellations, [pending, echo](Context &context) {
    auto child = context.call(pending, harmos::conformance::Value{});
    context.call(echo, value("release")).get();
    child.cancel();
    context.call(pending, harmos::conformance::Value{});
    return value("cancelled");
  });
  plugin.stream(cancellations).observer(cancel_observer.every(interval));

  const harmos::plugin::Operation<InvocationsCreateRequest, InvocationsCreateResponse> create("application.host.v1.Invocations/Create",
                                                                                              harmos::VISIBILITY_PUBLIC);
  const auto refuse = [create](Context &context, harmos::conformance::Value input) {
    InvocationsCreateRequest request;
    if (!request.ParseFromArray(harmos::conformance::caller_request.data(), static_cast<int>(harmos::conformance::caller_request.size()))) {
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
  plugin
      .operation(harmos::plugin::Operation<harmos::conformance::Value, harmos::conformance::Value>("conformance.caller_refusal",
                                                                                                   harmos::VISIBILITY_PUBLIC, refuse))
      .requires_operation(create);
  harmos::plugin::Stream<harmos::conformance::Value> refusals("conformance.window_caller_refusal");
  harmos::plugin::Observer<harmos::conformance::Value> refusal_observer(refusals, [refuse](Context &context) {
    return refuse(context, harmos::conformance::Value{});
  });
  plugin.stream(refusals).observer(refusal_observer.every(interval));

  // Answer a Whole Window at Once: Each harmos::conformance::Value Holds Its Sample's Sequence.
  harmos::plugin::Stream<harmos::conformance::Value> window("conformance.window");
  auto columns = harmos::plugin::Observer<harmos::conformance::Value>::window<::conformance::model::v1::Values>(
      window, [](Context &, const harmos::plugin::Window &grant) {
        ::conformance::model::v1::Values result;
        for (uint64_t index = 0; index < grant.count; ++index) {
          const auto sequence = grant.sequence + index;
          string bytes(8, '\0');
          for (unsigned position = 0; position < 8; ++position)
            bytes[position] = static_cast<char>(sequence >> (8 * position));
          result.add_value(std::move(bytes));
        }
        return result;
      });
  plugin.stream(window).observer(columns.every(interval));

  // Each Sample Holds as Many Bytes as Its Sequence, So a Grant Can Exceed the Message Size.
  harmos::plugin::Stream<harmos::conformance::Value> oversized("conformance.oversized");
  harmos::plugin::Observer<harmos::conformance::Value> oversized_observer(oversized, [](Context &ctx) {
    harmos::conformance::Value result;
    result.set_value(string(static_cast<size_t>(ctx.sequence), static_cast<char>(0)));
    return result;
  });
  plugin.stream(oversized).observer(oversized_observer.every(interval));

  return plugin;
}
