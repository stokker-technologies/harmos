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
#include <iostream>

// Third Party Imports

// Local Imports
#include "harmos/harmos.hpp"
#include "harmos/conformance/v1/fixture.pb.h"

/* === Macros === */

/* === Constants === */

/* === Types === */

using Graph = harmos::conformance::v1::Graph;
using Value = harmos::conformance::v1::Value;
using Operation = harmos::Operation<Value, Value>;
using harmos::Context;

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

int install_itself(int argc, char** argv);

/* === Private Functions === */

Value value(std::string bytes) {
    Value result;
    result.set_value(std::move(bytes));
    return result;
}

harmos::wire::Failure failure() {
    harmos::wire::Failure failure;
    failure.set_classification(harmos::wire::CLASSIFICATION_PERMANENT);
    failure.set_code("fixture_failure");
    failure.set_message("Fixture Failed");
    failure.set_action("Use Another Input");
    failure.set_operation("conformance.failure");
    failure.set_plugin("conformance");
    (*failure.mutable_details())["fixture"] = "value";
    failure.set_diagnostics("Fixture Diagnostics");
    auto* cause = failure.mutable_cause();
    cause->set_classification(harmos::wire::CLASSIFICATION_TRANSIENT);
    cause->set_code("fixture_cause");
    cause->set_message("Fixture Cause");
    cause->set_action("Retry");
    cause->set_operation("conformance.host.fail");
    cause->set_plugin("host");
    return failure;
}

/* === Entry Point === */

int main(int argc, char** argv) {
    try {
        if (argc > 1 && std::string(argv[1]) == "--install-itself") return install_itself(argc, argv);
        // Compose Typed Operations and Their Required Host Contracts.
        const Operation echo("conformance.host.echo", harmos::wire::VISIBILITY_PUBLIC);
        const Operation pending("conformance.host.pending", harmos::wire::VISIBILITY_PUBLIC);
        const Operation fail("conformance.host.fail", harmos::wire::VISIBILITY_PUBLIC);
        Operation local("conformance.local", harmos::wire::VISIBILITY_PRIVATE, [echo](Context& context, Value request) {
            if (request.value() == "deadline" || request.value() == "overall") { google::protobuf::Duration delay; delay.set_seconds(3600); context.sleep(delay); return request; }
            if (request.value() == "cancel") { context.call(echo, request).get(); return value(context.cancelled()); }
            if (request.value() != "retry" && request.value() != "retry-bound") return request;
            if (context.attempt == 1) { auto error = failure(); error.set_classification(harmos::wire::CLASSIFICATION_TRANSIENT); throw harmos::Failure(error); }
            return value(std::to_string(context.attempt));
        });
        local.replay_safe();
        harmos::Plugin plugin("conformance", "1.0.0");
        plugin.operation(local).operation(Operation("conformance.local_call", harmos::wire::VISIBILITY_PUBLIC,
            [local](Context& ctx, Value request) {
                std::optional<harmos::wire::OperationPolicy> policy;
                if (request.value() == "deadline") { policy.emplace(); policy->mutable_timeout()->set_seconds(3); policy->mutable_attempt_timeout()->set_seconds(2); }
                if (request.value() == "cancel") { policy.emplace(); policy->mutable_cancellation_timeout()->set_seconds(1); }
                if (request.value() == "overall") { policy.emplace(); policy->mutable_timeout()->set_seconds(2); policy->mutable_cancellation_timeout()->set_seconds(1); }
                if (request.value() == "retry-bound") { policy.emplace(); policy->mutable_timeout()->set_seconds(1); policy->mutable_retries()->set_total_attempts(2); policy->mutable_retries()->mutable_initial()->set_seconds(3600); }
                if (request.value() == "retry") { policy.emplace(); auto* retry = policy->mutable_retries(); retry->set_total_attempts(2); retry->mutable_initial()->set_seconds(1); retry->mutable_maximum()->set_seconds(4); retry->set_deterministic_jitter(true); }
                return ctx.call(local, request, policy).get();
            }));
        plugin.operation(Operation("conformance.step.0", harmos::wire::VISIBILITY_PUBLIC, [](Context& ctx, Value request) { return request.value() == "scope" ? value(ctx.scope()) : request; })
                .visibility(harmos::wire::VISIBILITY_PUBLIC))
            .operation(Operation("conformance.children", harmos::wire::VISIBILITY_PUBLIC, [echo](Context& ctx, Value) {
                auto first = ctx.call(echo, value(std::string(1, '\1')));
                auto second = ctx.call(echo, value(std::string(1, '\2')));
                return value(first.get().value() + second.get().value());
            }).visibility(harmos::wire::VISIBILITY_PUBLIC))
            .operation(Operation("conformance.cancel", harmos::wire::VISIBILITY_PUBLIC, [pending](Context& ctx, Value request) { return ctx.call(pending, request).get(); })
                .visibility(harmos::wire::VISIBILITY_PUBLIC))
            .operation(Operation("conformance.failure", harmos::wire::VISIBILITY_PUBLIC, [](Context&, Value) -> Value { throw harmos::Failure(failure()); })
                .visibility(harmos::wire::VISIBILITY_PUBLIC))
            .operation(Operation("conformance.child_failure", harmos::wire::VISIBILITY_PUBLIC, [fail](Context& ctx, Value request) { return ctx.call(fail, request).get(); })
                .visibility(harmos::wire::VISIBILITY_PUBLIC))
            .requires_operation(echo).requires_operation(pending).requires_operation(fail);

        plugin.requires_operation(harmos::Operation<Graph, Value>("conformance.host.schema", harmos::wire::VISIBILITY_PUBLIC));
        plugin.operation(Operation("conformance.policy", harmos::wire::VISIBILITY_PUBLIC, [echo](Context& ctx, Value request) {
            harmos::wire::OperationPolicy policy;
            policy.mutable_retries()->set_total_attempts(3);
            policy.mutable_retries()->mutable_initial()->set_seconds(1);
            policy.mutable_retries()->mutable_maximum()->set_seconds(4);
            policy.mutable_retries()->set_deterministic_jitter(true);
            const auto first = ctx.call(echo, request, policy).get();
            return ctx.call(echo, first).get();
        }).visibility(harmos::wire::VISIBILITY_PUBLIC));
        plugin.operation(harmos::Operation<Graph, Graph::Nested>("conformance.schema", harmos::wire::VISIBILITY_PUBLIC, [](Context&, Graph request) {
            Graph::Nested response;
            response.set_count(request.items_size());
            *response.mutable_next() = std::move(request);
            return response;
        }).visibility(harmos::wire::VISIBILITY_PUBLIC));
        plugin.operation(harmos::Operation<Graph::Nested, Graph>("conformance.nested", harmos::wire::VISIBILITY_PUBLIC, [](Context&, Graph::Nested request) {
            return request.next();
        }).visibility(harmos::wire::VISIBILITY_PUBLIC));

        plugin.operation(harmos::Operation<Graph, Value>("conformance.projected", harmos::wire::VISIBILITY_PUBLIC, [](Context&, Graph request) {
            return request.tail().value();
        }).visibility(harmos::wire::VISIBILITY_PUBLIC));

        plugin.requires_operation(harmos::SLEEP).requires_operation(harmos::GET_TIME);
        plugin.operation(Operation("conformance.clock", harmos::wire::VISIBILITY_PUBLIC, [](Context& ctx, Value) {
            const auto before = ctx.now();
            google::protobuf::Duration duration;
            duration.set_seconds(3600);
            ctx.sleep(duration);
            const auto after = ctx.now();
            const auto elapsed = static_cast<std::uint64_t>(after.seconds() - before.seconds());
            std::string bytes(8, '\0');
            for (unsigned index = 0; index < 8; ++index) bytes[index] = static_cast<char>(elapsed >> (8 * index));
            return value(bytes);
        }).visibility(harmos::wire::VISIBILITY_PUBLIC));

        plugin.operation(Operation("example.plugin.v1.Lifecycle/Initialize", harmos::wire::VISIBILITY_PROTECTED, [](Context&, Value request) { return request; }));
        plugin.operation(Operation("example.plugin.v1.Steps/Execute", harmos::wire::VISIBILITY_PUBLIC, [echo](Context& ctx, Value request) {
            ctx.set(request);
            ctx.call(echo, request).get();
            return ctx.get<Value>();
        }).visibility(harmos::wire::VISIBILITY_PUBLIC).replay_safe());
        plugin.operation(Operation("conformance.cancel_reason", harmos::wire::VISIBILITY_PUBLIC, [echo](Context& ctx, Value request) {
            ctx.call(echo, request).get();
            return value(ctx.cancelled());
        }).visibility(harmos::wire::VISIBILITY_PUBLIC));

        // Compose a Host-Ticked Observer and a Device-Driven Stream.
        harmos::Stream<Value> ticks("conformance.ticks");
        harmos::Observer<Value> observer(ticks, [](Context& ctx) {
            std::string bytes(8, '\0');
            for (unsigned index = 0; index < 8; ++index) bytes[index] = static_cast<char>(ctx.sequence >> (8 * index));
            return value(bytes);
        });
        Operation host_watch("conformance.host.watch", harmos::wire::VISIBILITY_PUBLIC);
        host_watch.watch();
        Operation watch("conformance.watch", harmos::wire::VISIBILITY_PUBLIC, [host_watch](Context& ctx, Value request) {
            auto watching = ctx.watch(host_watch, request, 4096);
            std::string result;
            while (auto item = watching.next()) {
                result += item->value();
                if (request.value() == "cancel") break;
            }
            return value(result);
        });
        plugin.operation(watch).requires_operation(host_watch);
        const Operation peer_public("conformance.peer.public", harmos::wire::VISIBILITY_PUBLIC);
        const Operation peer_protected("conformance.peer.protected", harmos::wire::VISIBILITY_PROTECTED);
        const Operation peer_local("conformance.peer.local", harmos::wire::VISIBILITY_PROTECTED);
        const Operation own_protected("example.plugin.v1.Lifecycle/Initialize", harmos::wire::VISIBILITY_PROTECTED);
        plugin.operation(Operation("conformance.peer_call", harmos::wire::VISIBILITY_PUBLIC,
            [peer_public, peer_protected, peer_local, own_protected](Context& context, Value request) {
                if (request.value() == "self") return context.call(own_protected, request).get();
                if (request.value() == "public") return context.call(peer_public, value(context.scope())).get();
                return context.call(request.value() == "protected" ? peer_protected : peer_local, request).get();
            })).requires_operation(peer_public).requires_operation(peer_protected).requires_operation(peer_local).requires_operation(own_protected);
        google::protobuf::Duration interval;
        interval.set_seconds(1);
        observer.every(interval);
        harmos::Stream<Value> device("conformance.device");
        device.publish_on([](Context& ctx, harmos::Publisher<Value> publisher) {
            publisher.publish(value("device"));
            ctx.cancelled();
        });
        harmos::Stream<Value> settings("conformance.settings");
        harmos::Observer<Value> configured(settings, [](Context& ctx) {
            std::string bytes(8, '\0');
            for (unsigned index = 0; index < 8; ++index) bytes[index] = static_cast<char>(ctx.sequence >> (8 * index));
            return value(bytes);
        });
        configured.every(interval).interval_setting("harmos.conformance.v1.Graph.interval");
        Graph section;
        section.mutable_interval()->set_seconds(2);
        plugin.message<harmos::conformance::v1::PayloadOnly>().message<harmos::conformance::v1::PayloadOnly>();
        plugin.stream(ticks).stream(device).stream(settings).observer(observer).observer(configured).section(section);

        // Serve as a Managed Executable or at an Explicit Remote Address.
        if (argc == 3 && std::string(argv[1]) == "--address") plugin.serve(argv[2]);
        else if (argc == 1) plugin.serve();
        else throw std::invalid_argument("Usage: harmos-cpp-conformance [--address ADDRESS]");
        return 0;
    } catch (const std::exception& error) {
        std::cerr << error.what() << std::endl;
        return 1;
    }
}
