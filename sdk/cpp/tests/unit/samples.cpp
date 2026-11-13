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
#include <future>

// Third Party Imports
#include <catch2/catch_test_macros.hpp>

// Local Imports
#include "samples.hpp"
#include "harmos/plugin/v1/instance.grpc.pb.h"
#include "harmos/conformance/v1/fixture.pb.h"

/* === Macros === */

/* === Constants === */

/* === Types === */

namespace {
using harmos::Context;
using harmos::conformance::v1::Value;
using harmos::plugin::v1::Instance;
using harmos::plugin::v1::InstanceSamplesRequest;
using harmos::plugin::v1::InstanceSamplesResponse;
using ValueOperation = harmos::Operation<Value, Value>;
const ValueOperation echo("example.host.v1.Echo/Call", harmos::wire::VISIBILITY_PUBLIC);
const ValueOperation watching = ValueOperation("example.host.v1.Events/Watch", harmos::wire::VISIBILITY_PUBLIC).watch();

/* === Classes === */

class SamplesHost final : public Instance::CallbackService {
public:
    std::vector<harmos::detail::StreamDefinition> streams;
    std::vector<harmos::detail::OperationDefinition> operations;
    std::map<std::string, std::string> requirements;
    std::unique_ptr<grpc::Server> server;
    std::shared_ptr<grpc::Channel> channel;
    std::mutex mutex;
    std::condition_variable changed;
    std::size_t opened = 0;
    std::size_t closed = 0;

    explicit SamplesHost(std::function<std::string(Context&)> observer, bool device = false) {
        harmos::detail::StreamDefinition stream;
        stream.declaration.set_id("example.rows");
        if (device) stream.producer = [observer](Context& context, auto) { observer(context); };
        else stream.observer = std::move(observer);
        streams.push_back(std::move(stream));
        const auto catalog = harmos::Plugin("example", "1.0.0").requires_operation(echo).requires_operation(watching)
            .requires_operation(harmos::GET_TIME).requires_operation(harmos::LIST_SETTINGS).catalog();
        for (const auto& requirement : catalog.requires_()) requirements[requirement.id()] = requirement.fingerprint();
        grpc::ServerBuilder builder;
        int port = 0;
        builder.AddListeningPort("127.0.0.1:0", grpc::InsecureServerCredentials(), &port);
        builder.RegisterService(this);
        server = builder.BuildAndStart();
        if (!server) throw std::runtime_error("Serve Samples: Bind Failed");
        channel = grpc::CreateChannel("127.0.0.1:" + std::to_string(port), grpc::InsecureChannelCredentials());
    }
    ~SamplesHost() {
        server->Shutdown();
        server->Wait();
        std::unique_lock lock(mutex);
        changed.wait(lock, [&] { return opened == closed; });
    }
    grpc::ServerBidiReactor<InstanceSamplesRequest, InstanceSamplesResponse>* Samples(grpc::CallbackServerContext*) override {
        auto* call = new harmos::detail::PublicationCall(streams, operations, requirements, "example");
        { std::lock_guard lock(mutex); ++opened; }
        call->settled = [this] {
            std::lock_guard lock(mutex);
            ++closed;
            changed.notify_all();
        };
        return call;
    }
};

class Samples {
public:
    grpc::ClientContext context;
    std::unique_ptr<grpc::ClientReaderWriter<InstanceSamplesRequest, InstanceSamplesResponse>> stream;
    bool finished = false;
    explicit Samples(const SamplesHost& host) : stream(Instance::NewStub(host.channel)->Samples(&context)) {
        InstanceSamplesRequest request;
        request.mutable_open()->set_stream("example.rows");
        request.mutable_open()->set_scope("opaque-scope");
        REQUIRE(stream->Write(request));
    }
    ~Samples() { if (!finished) { context.TryCancel(); stream->Finish(); } }
    void tick(std::int64_t seconds) {
        InstanceSamplesRequest request;
        request.mutable_tick()->mutable_at()->set_seconds(seconds);
        REQUIRE(stream->Write(request));
    }
    InstanceSamplesResponse read() {
        InstanceSamplesResponse response;
        REQUIRE(stream->Read(&response));
        return response;
    }
    void reply(std::uint64_t call, const google::protobuf::Message& value) {
        InstanceSamplesRequest request;
        request.mutable_child()->set_call(call);
        request.mutable_child()->set_payload(harmos::detail::encode(value));
        REQUIRE(stream->Write(request));
    }
    grpc::Status finish() {
        stream->WritesDone();
        const auto result = stream->Finish();
        finished = true;
        return result;
    }
};

/* === Templates === */

/* === Public Functions === */

Value value(std::string bytes) { Value result; result.set_value(std::move(bytes)); return result; }
}

TEST_CASE("Samples Correlate Reverse Replies Across Ticks and Streams") {
    SamplesHost host([](Context& context) {
        auto first = context.call(echo, value("first"));
        auto second = context.call(echo, value("second"));
        return harmos::detail::encode(value(first.get().value() + second.get().value()));
    });
    Samples first(host);
    Samples second(host);
    for (std::int64_t tick = 1; tick <= 2; ++tick) {
        first.tick(tick);
        second.tick(tick);
        const auto one = first.read();
        const auto two = first.read();
        const auto other_one = second.read();
        const auto other_two = second.read();
        REQUIRE(one.samples().empty());
        REQUIRE(two.samples().empty());
        REQUIRE(one.child().call() == static_cast<std::uint64_t>(tick * 2 - 1));
        REQUIRE(two.child().call() == static_cast<std::uint64_t>(tick * 2));
        REQUIRE(other_one.child().call() == one.child().call());
        REQUIRE(other_two.child().call() == two.child().call());
        first.reply(two.child().call(), value("B"));
        second.reply(other_two.child().call(), value("Y"));
        second.reply(other_one.child().call(), value("X"));
        first.reply(one.child().call(), value("A"));
        const auto sample = first.read();
        const auto other_sample = second.read();
        REQUIRE(sample.frame_case() == InstanceSamplesResponse::FRAME_NOT_SET);
        REQUIRE(sample.samples(0).sequence() == static_cast<std::uint64_t>(tick));
        REQUIRE(sample.samples(0).at().seconds() == tick);
        REQUIRE(harmos::decode<Value>(sample.samples(0).row()).value() == "AB");
        REQUIRE(harmos::decode<Value>(other_sample.samples(0).row()).value() == "XY");
    }
    REQUIRE(first.finish().ok());
    REQUIRE(second.finish().ok());
}

TEST_CASE("A Tick Calls the Clock Settings and a Demand-Bounded Watch") {
    SamplesHost host([](Context& context) {
        const auto now = context.now();
        const auto settings = context.settings();
        auto watch = context.watch(watching, value(context.scope()), 4096);
        std::string result = std::to_string(now.seconds()) + settings.value().values().at("label").text();
        while (auto item = watch.next()) result += item->value();
        return harmos::detail::encode(value(result));
    });
    Samples samples(host);
    samples.tick(10);
    auto child = samples.read();
    REQUIRE(child.child().operation() == "harmos.plugin.host.v1.Time/Get");
    harmos::plugin::host::v1::TimeGetResponse now;
    now.mutable_now()->set_seconds(42);
    samples.reply(child.child().call(), now);
    child = samples.read();
    REQUIRE(child.child().operation() == "harmos.plugin.host.v1.Settings/List");
    harmos::plugin::host::v1::SettingsListResponse settings;
    (*settings.mutable_value()->mutable_values())["label"].set_text("configured");
    samples.reply(child.child().call(), settings);
    child = samples.read();
    REQUIRE(child.child().watch());
    REQUIRE(child.child().max_response_bytes() == 4096);
    REQUIRE(harmos::decode<Value>(child.child().payload()).value() == "opaque-scope");
    const auto call = child.child().call();
    REQUIRE(samples.read().child_demand().call() == call);
    InstanceSamplesRequest item;
    item.mutable_child()->set_call(call);
    item.mutable_child()->set_item(harmos::detail::encode(value("item")));
    REQUIRE(samples.stream->Write(item));
    REQUIRE(samples.read().child_demand().call() == call);
    samples.reply(call, Value{});
    REQUIRE(harmos::decode<Value>(samples.read().samples(0).row()).value() == "42configureditem");
    REQUIRE(samples.finish().ok());
}

TEST_CASE("A Tick Preserves Every Structured Child Failure Field") {
    SamplesHost host([](Context& context) {
        try { context.call(echo, Value{}).get(); }
        catch (const harmos::Failure& failure) { return harmos::detail::encode(failure.value); }
        throw std::runtime_error("Call Echo: Expected Failure");
    });
    Samples samples(host);
    samples.tick(1);
    const auto child = samples.read();
    harmos::wire::Failure failure;
    failure.set_classification(harmos::wire::CLASSIFICATION_REFUSED);
    failure.set_code("refused");
    failure.set_message("Child Refused");
    failure.set_operation(child.child().operation());
    failure.set_plugin("example");
    failure.set_action("Retry");
    failure.set_diagnostics("diagnostic");
    (*failure.mutable_details())["field"] = "value";
    *failure.mutable_cause() = failure;
    InstanceSamplesRequest response;
    response.mutable_child()->set_call(child.child().call());
    *response.mutable_child()->mutable_failure() = failure;
    REQUIRE(samples.stream->Write(response));
    const auto actual = harmos::decode<harmos::wire::Failure>(samples.read().samples(0).row());
    REQUIRE(harmos::detail::encode(actual) == harmos::detail::encode(failure));
    REQUIRE(samples.finish().ok());
}

TEST_CASE("Tick Completion Cancels Children and Rejects Late Replies") {
    SamplesHost host([](Context& context) {
        auto cancelled = context.call(echo, Value{});
        cancelled.cancel();
        context.call(echo, Value{});
        return harmos::detail::encode(Value{});
    });
    Samples samples(host);
    samples.tick(1);
    const auto first = samples.read().child().call();
    REQUIRE(samples.read().child_cancel().call() == first);
    const auto second = samples.read().child().call();
    REQUIRE(second != first);
    REQUIRE(samples.read().child_cancel().call() == second);
    REQUIRE(samples.read().samples_size() == 1);
    samples.reply(second, Value{});
    InstanceSamplesResponse ignored;
    REQUIRE_FALSE(samples.stream->Read(&ignored));
    REQUIRE(samples.finish().error_code() == grpc::StatusCode::INVALID_ARGUMENT);
}

TEST_CASE("Samples Refuse Outside-Tick and Duplicate Child Traffic") {
    SECTION("A Reply Before a Tick Is Refused") {
        SamplesHost host([](Context&) { return std::string{}; });
        Samples samples(host);
        samples.reply(1, Value{});
        InstanceSamplesResponse response;
        REQUIRE_FALSE(samples.stream->Read(&response));
        REQUIRE(samples.finish().error_code() == grpc::StatusCode::INVALID_ARGUMENT);
    }
    SECTION("A Publisher Cannot Call Host Operations") {
        SamplesHost host([](Context& context) { return context.call(echo, Value{}).get().value(); }, true);
        Samples samples(host);
        InstanceSamplesResponse response;
        REQUIRE_FALSE(samples.stream->Read(&response));
        REQUIRE(samples.finish().error_code() == grpc::StatusCode::INVALID_ARGUMENT);
    }
    SECTION("A Duplicate Reply Cannot Satisfy Another Child") {
        SamplesHost host([](Context& context) {
            context.call(echo, Value{}).get();
            return context.call(echo, Value{}).get().value();
        });
        Samples samples(host);
        samples.tick(1);
        const auto call = samples.read().child().call();
        samples.reply(call, Value{});
        REQUIRE(samples.read().child().call() != call);
        samples.reply(call, Value{});
        InstanceSamplesResponse response;
        REQUIRE_FALSE(samples.stream->Read(&response));
        REQUIRE(samples.finish().error_code() == grpc::StatusCode::INVALID_ARGUMENT);
    }
}

TEST_CASE("Ending Samples Settles Open Child Calls") {
    std::promise<void> settled;
    auto done = settled.get_future();
    SamplesHost host([&settled](Context& context) {
        try { context.call(echo, Value{}).get(); }
        catch (const harmos::TransportError&) { settled.set_value(); throw; }
        return std::string{};
    });
    Samples samples(host);
    samples.tick(1);
    REQUIRE(samples.read().has_child());
    REQUIRE(samples.finish().ok());
    done.get();
}

/* === Private Functions === */

/* === Entry Point === */
