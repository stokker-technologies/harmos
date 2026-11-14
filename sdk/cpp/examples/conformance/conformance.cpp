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

namespace authoring = harmos::plugin;
namespace fixture_types = harmos::conformance;
namespace sdk = harmos;
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

using Graph = harmos::conformance::Graph;
using Value = harmos::conformance::Value;
using ValueOperation = harmos::plugin::Operation<Value, Value>;
using harmos::host::v1::InvocationsCreateRequest;
using harmos::host::v1::InvocationsCreateResponse;
using harmos::plugin::Context;

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

/* === Private Functions === */

namespace {
Value value(string bytes) {
  Value result;
  result.set_value(std::move(bytes));
  return result;
}

sdk::FailureValue failure() {
  sdk::FailureValue failure;
  failure.set_classification(sdk::CLASSIFICATION_PERMANENT);
  failure.set_code("fixture_failure");
  failure.set_message("Fixture Failed");
  failure.set_action("Use Another Input");
  failure.set_operation("conformance.failure");
  failure.set_plugin("conformance");
  (*failure.mutable_details())["fixture"] = "value";
  failure.set_diagnostics("Fixture Diagnostics");
  auto *cause = failure.mutable_cause();
  cause->set_classification(sdk::CLASSIFICATION_TRANSIENT);
  cause->set_code("fixture_cause");
  cause->set_message("Fixture Cause");
  cause->set_action("Retry");
  cause->set_operation("conformance.host.fail");
  cause->set_plugin("host");
  return failure;
}

/* === Entry Point === */

} // namespace

authoring::Plugin fixture_types::plugin() {
  // Compose Typed Operations and Their Required Host Contracts.
  const ValueOperation echo("conformance.host.echo", sdk::VISIBILITY_PUBLIC);
  const ValueOperation pending("conformance.host.pending", sdk::VISIBILITY_PUBLIC);
  const ValueOperation fail("conformance.host.fail", sdk::VISIBILITY_PUBLIC);
  ValueOperation local("conformance.local", sdk::VISIBILITY_PRIVATE, [echo](Context &context, Value request) {
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
      error.set_classification(sdk::CLASSIFICATION_TRANSIENT);
      throw sdk::Failure(error);
    }
    return value(std::to_string(context.attempt));
  });
  local.replay_safe();
  authoring::Plugin plugin("conformance", "1.0.0");
  plugin.operation(local).operation(ValueOperation("conformance.local_call", sdk::VISIBILITY_PUBLIC, [local](Context &ctx, Value request) {
    optional<sdk::OperationPolicy> policy;
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
      .operation(ValueOperation("conformance.step.0", sdk::VISIBILITY_PUBLIC,
                                [](Context &ctx, Value request) {
                                  return request.value() == "scope" ? value(ctx.scope()) : request;
                                })
                     .visibility(sdk::VISIBILITY_PUBLIC))
      .operation(ValueOperation("conformance.children", sdk::VISIBILITY_PUBLIC,
                                [echo](Context &ctx, Value) {
                                  auto first = ctx.call(echo, value(string(1, '\1')));
                                  auto second = ctx.call(echo, value(string(1, '\2')));
                                  return value(first.get().value() + second.get().value());
                                })
                     .visibility(sdk::VISIBILITY_PUBLIC))
      .operation(ValueOperation("conformance.cancel", sdk::VISIBILITY_PUBLIC,
                                [pending](Context &ctx, Value request) {
                                  return ctx.call(pending, request).get();
                                })
                     .visibility(sdk::VISIBILITY_PUBLIC))
      .operation(ValueOperation("conformance.failure", sdk::VISIBILITY_PUBLIC,
                                [](Context &, Value) -> Value {
                                  throw sdk::Failure(failure());
                                })
                     .visibility(sdk::VISIBILITY_PUBLIC))
      .operation(ValueOperation("conformance.child_failure", sdk::VISIBILITY_PUBLIC,
                                [fail](Context &ctx, Value request) {
                                  return ctx.call(fail, request).get();
                                })
                     .visibility(sdk::VISIBILITY_PUBLIC))
      .requires_operation(echo)
      .requires_operation(pending)
      .requires_operation(fail);

  plugin.requires_operation(authoring::Operation<Graph, Value>("conformance.host.schema", sdk::VISIBILITY_PUBLIC));
  plugin.operation(ValueOperation("conformance.policy", sdk::VISIBILITY_PUBLIC, [echo](Context &ctx, Value request) {
                     sdk::OperationPolicy policy;
                     policy.mutable_retries()->set_total_attempts(3);
                     policy.mutable_retries()->mutable_initial()->set_seconds(1);
                     policy.mutable_retries()->mutable_maximum()->set_seconds(4);
                     policy.mutable_retries()->set_deterministic_jitter(true);
                     const auto first = ctx.call(echo, request, policy).get();
                     return ctx.call(echo, first).get();
                   }).visibility(sdk::VISIBILITY_PUBLIC));
  plugin.operation(authoring::Operation<Graph, Graph::Nested>("conformance.schema", sdk::VISIBILITY_PUBLIC, [](Context &, Graph request) {
                     Graph::Nested response;
                     response.set_count(request.items_size());
                     *response.mutable_next() = std::move(request);
                     return response;
                   }).visibility(sdk::VISIBILITY_PUBLIC));
  plugin.operation(
      authoring::Operation<Graph::Nested, Graph>("conformance.nested", sdk::VISIBILITY_PUBLIC, [](Context &, Graph::Nested request) {
        return request.next();
      }).visibility(sdk::VISIBILITY_PUBLIC));

  plugin.operation(authoring::Operation<Graph, Value>("conformance.projected", sdk::VISIBILITY_PUBLIC, [](Context &, Graph request) {
                     return request.tail().value();
                   }).visibility(sdk::VISIBILITY_PUBLIC));

  plugin.requires_operation(authoring::HostOperation::TimeSleep).requires_operation(authoring::HostOperation::TimeGet);
  plugin.requires_operation(authoring::HostOperation::PluginsSettingsList);
  plugin.requires_operation(authoring::HostOperation::OperationsList).requires_operation(authoring::HostOperation::OperationsGet);
  plugin.operation(ValueOperation("conformance.clock", sdk::VISIBILITY_PUBLIC, [](Context &ctx, Value) {
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
                   }).visibility(sdk::VISIBILITY_PUBLIC));

  plugin.operation(ValueOperation(lifecycle_operation, sdk::VISIBILITY_PROTECTED, [](Context &, Value request) {
    return request;
  }));
  plugin.operation(ValueOperation(steps_operation, sdk::VISIBILITY_PUBLIC,
                                  [echo](Context &ctx, Value request) {
                                    ctx.set(request);
                                    ctx.call(echo, request).get();
                                    return ctx.get<Value>();
                                  })
                       .visibility(sdk::VISIBILITY_PUBLIC)
                       .replay_safe());
  plugin.operation(ValueOperation("conformance.cancel_reason", sdk::VISIBILITY_PUBLIC, [echo](Context &ctx, Value request) {
                     ctx.call(echo, request).get();
                     return value(ctx.cancelled());
                   }).visibility(sdk::VISIBILITY_PUBLIC));

  // Compose a Host-Ticked Observer and a Device-Driven Stream.
  authoring::Stream<Value> ticks("conformance.ticks");
  authoring::Observer<Value> observer(ticks, [](Context &ctx) {
    string bytes(8, '\0');
    for (unsigned index = 0; index < 8; ++index)
      bytes[index] = static_cast<char>(ctx.sequence >> (8 * index));
    return value(bytes);
  });
  ValueOperation host_watch("conformance.host.watch", sdk::VISIBILITY_PUBLIC);
  host_watch.watch();
  ValueOperation watch("conformance.watch", sdk::VISIBILITY_PUBLIC, [host_watch](Context &ctx, Value request) {
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
  const ValueOperation peer_public("conformance.peer.public", sdk::VISIBILITY_PUBLIC);
  const ValueOperation peer_protected("conformance.peer.protected", sdk::VISIBILITY_PROTECTED);
  const ValueOperation peer_local("conformance.peer.local", sdk::VISIBILITY_PROTECTED);
  const ValueOperation own_protected(lifecycle_operation, sdk::VISIBILITY_PROTECTED);
  plugin
      .operation(ValueOperation("conformance.peer_call", sdk::VISIBILITY_PUBLIC,
                                [peer_public, peer_protected, peer_local, own_protected](Context &context, Value request) {
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
  plugin.operation(ValueOperation("conformance.settings_read", sdk::VISIBILITY_PUBLIC, [](Context &context, Value) {
    return value(authoring::detail::encode(context.host().plugins.settings.list().value()));
  }));
  plugin.operation(ValueOperation("conformance.discover", sdk::VISIBILITY_PUBLIC, [](Context &context, Value) {
    const auto listed = context.host().operations.list();
    if (listed.empty())
      throw runtime_error("List Operations: Missing Contract");
    const auto &operation = listed.at(0);
    const auto found = context.host().operations.get(operation.contract().id());
    if (authoring::detail::encode(found) != authoring::detail::encode(operation))
      throw runtime_error("Get Operation: Contract Differs");
    return value(operation.contract().id());
  }));

  Duration interval;
  interval.set_seconds(1);
  observer.every(interval);
  authoring::Stream<Value> device("conformance.device");
  device.publish_on([](Context &ctx, authoring::Publisher<Value> publisher) {
    publisher.publish(value("device"));
    ctx.cancelled();
  });
  authoring::Stream<Value> settings("conformance.settings");
  authoring::Observer<Value> configured(settings, [](Context &ctx) {
    string bytes(8, '\0');
    for (unsigned index = 0; index < 8; ++index)
      bytes[index] = static_cast<char>(ctx.sequence >> (8 * index));
    return value(bytes);
  });
  configured.every(interval).interval_setting("interval");
  Graph section;
  section.mutable_interval()->set_seconds(2);
  plugin.message<fixture_types::PayloadOnly>().message<fixture_types::PayloadOnly>();
  plugin.stream(ticks).stream(device).stream(settings).observer(observer).observer(configured).section(section);

  for (const auto *name : {"count", "enabled", "gain", "interval", "label"}) {
    sdk::SettingDeclaration setting;
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

  authoring::Stream<Value> children("conformance.tick_children");
  authoring::Observer<Value> child_observer(children, [echo](Context &context) {
    auto first = context.call(echo, value(string(1, '\1')));
    auto second = context.call(echo, value(string(1, '\2')));
    return value(first.get().value() + second.get().value());
  });
  plugin.stream(children).observer(child_observer.every(interval));

  authoring::Stream<Value> calls("conformance.tick_context");
  authoring::Observer<Value> call_observer(calls, [echo](Context &context) {
    const auto echoed = context.call(echo, value(context.scope())).get();
    sdk::TimeGetResponse time;
    *time.mutable_now() = context.host().time.get();
    const auto settings = context.host().plugins.settings.list();
    string bytes;
    for (const auto &reply : {authoring::detail::encode(echoed), authoring::detail::encode(time), authoring::detail::encode(settings)}) {
      const auto length = static_cast<uint32_t>(reply.size());
      for (unsigned index = 0; index < 4; ++index)
        bytes += static_cast<char>(length >> (8 * index));
      bytes += reply;
    }
    return value(bytes);
  });
  plugin.stream(calls).observer(call_observer.every(interval));

  authoring::Stream<Value> failures("conformance.tick_failure");
  authoring::Observer<Value> failure_observer(failures, [fail](Context &context) {
    try {
      context.call(fail, Value{}).get();
    } catch (const sdk::Failure &failure) {
      return value(authoring::detail::encode(failure.value));
    }
    throw runtime_error("Sample Failure: Expected Child Failure");
  });
  plugin.stream(failures).observer(failure_observer.every(interval));

  authoring::Stream<Value> watches("conformance.tick_watch");
  authoring::Observer<Value> watch_observer(watches, [host_watch](Context &context) {
    auto watching = context.watch(host_watch, value(context.scope()), 4096);
    string result;
    while (auto item = watching.next())
      result += item->value();
    return value(result);
  });
  plugin.stream(watches).observer(watch_observer.every(interval));

  authoring::Stream<Value> cancellations("conformance.tick_cancel");
  authoring::Observer<Value> cancel_observer(cancellations, [pending, echo](Context &context) {
    auto child = context.call(pending, Value{});
    context.call(echo, value("release")).get();
    child.cancel();
    context.call(pending, Value{});
    return value("cancelled");
  });
  plugin.stream(cancellations).observer(cancel_observer.every(interval));

  const authoring::Operation<InvocationsCreateRequest, InvocationsCreateResponse> create("harmos.host.v1.Invocations/Create",
                                                                                         sdk::VISIBILITY_PUBLIC);
  const auto refuse = [create](Context &context, Value) {
    InvocationsCreateRequest request;
    if (!request.ParseFromArray(fixture_types::caller_request.data(), static_cast<int>(fixture_types::caller_request.size()))) {
      throw runtime_error("Create Invocation: Invalid Generated Probe Request");
    }
    try {
      context.call(create, request).get();
    } catch (const sdk::Failure &failure) {
      return value(authoring::detail::encode(failure.value));
    }
    throw runtime_error("Create Invocation: Client-Only Operation Was Admitted");
  };
  plugin.operation(ValueOperation("conformance.caller_refusal", sdk::VISIBILITY_PUBLIC, refuse)).requires_operation(create);
  authoring::Stream<Value> refusals("conformance.tick_caller_refusal");
  authoring::Observer<Value> refusal_observer(refusals, [refuse](Context &context) {
    return refuse(context, Value{});
  });
  plugin.stream(refusals).observer(refusal_observer.every(interval));

  return plugin;
}
