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
        // Compose Typed Operations and Their Required Host Contracts.
        const Operation echo("conformance.host.echo");
        const Operation pending("conformance.host.pending");
        const Operation fail("conformance.host.fail");
        harmos::Plugin plugin("conformance", "1.0.0");
        plugin.operation(Operation("conformance.echo", [](Context&, Value request) { return request; })
                .visibility(harmos::wire::VISIBILITY_PUBLIC))
            .operation(Operation("conformance.children", [echo](Context& ctx, Value) {
                auto first = ctx.call(echo, value(std::string(1, '\1')));
                auto second = ctx.call(echo, value(std::string(1, '\2')));
                return value(first.get().value() + second.get().value());
            }).visibility(harmos::wire::VISIBILITY_PUBLIC))
            .operation(Operation("conformance.cancel", [pending](Context& ctx, Value request) { return ctx.operation(pending, request); })
                .visibility(harmos::wire::VISIBILITY_PUBLIC))
            .operation(Operation("conformance.failure", [](Context&, Value) -> Value { throw harmos::Failure(failure()); })
                .visibility(harmos::wire::VISIBILITY_PUBLIC))
            .operation(Operation("conformance.child_failure", [fail](Context& ctx, Value request) { return ctx.operation(fail, request); })
                .visibility(harmos::wire::VISIBILITY_PUBLIC))
            .requires_operation(echo).requires_operation(pending).requires_operation(fail);

        plugin.requires_operation(harmos::Operation<Graph, Value>("conformance.host.schema"));
        plugin.operation(Operation("conformance.policy", [echo](Context& ctx, Value request) {
            harmos::wire::OperationPolicy policy;
            policy.set_attempts(3);
            const auto first = ctx.operation(echo, request, policy);
            return ctx.operation(echo, first);
        }).visibility(harmos::wire::VISIBILITY_PUBLIC));
        plugin.operation(harmos::Operation<Graph, Graph::Nested>("conformance.schema", [](Context&, Graph request) {
            Graph::Nested response;
            response.set_count(request.items_size());
            *response.mutable_next() = std::move(request);
            return response;
        }).visibility(harmos::wire::VISIBILITY_PUBLIC));
        plugin.operation(harmos::Operation<Graph::Nested, Graph>("conformance.nested", [](Context&, Graph::Nested request) {
            return request.next();
        }).visibility(harmos::wire::VISIBILITY_PUBLIC));

        plugin.operation(harmos::Operation<Graph, Value>("conformance.projected", [](Context&, Graph request) {
            return request.tail().value();
        }).visibility(harmos::wire::VISIBILITY_PUBLIC));

        plugin.requires_operation(harmos::SLEEP).requires_operation(harmos::NOW);
        plugin.operation(Operation("conformance.clock", [](Context& ctx, Value) {
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

        // Compose a Host-Ticked Observer and a Device-Driven Stream.
        harmos::Stream<Value> ticks("conformance.ticks");
        harmos::Observer<Value> observer(ticks, [](Context& ctx) {
            std::string bytes(8, '\0');
            for (unsigned index = 0; index < 8; ++index) bytes[index] = static_cast<char>(ctx.sequence >> (8 * index));
            return value(bytes);
        });
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
