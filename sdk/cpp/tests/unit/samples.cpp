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
#include "generated/types.hpp"
#include "harmos/plugin/v1/instance.grpc.pb.h"
#include "samples.hpp"

using google::protobuf::Message;
using grpc::CallbackServerContext;
using grpc::Channel;
using grpc::ClientContext;
using grpc::ClientReaderWriter;
using grpc::CreateChannel;
using grpc::InsecureChannelCredentials;
using grpc::InsecureServerCredentials;
using grpc::Server;
using grpc::ServerBidiReactor;
using grpc::ServerBuilder;
using grpc::Status;
using grpc::StatusCode;
using std::condition_variable;
using std::int64_t;
using std::lock_guard;
using std::map;
using std::mutex;
using std::promise;
using std::runtime_error;
using std::shared_ptr;
using std::size_t;
using std::string;
using std::uint64_t;
using std::unique_lock;
using std::unique_ptr;
using std::vector;

/* === Macros === */

/* === Constants === */

/* === Types === */

namespace {
using ::conformance::model::v1::Values;
using harmos::conformance::Value;
using harmos::plugin::Context;
using harmos::plugin::v1::Instance;
using harmos::plugin::v1::InstanceSamplesRequest;
using harmos::plugin::v1::InstanceSamplesResponse;
using ValueOperation = harmos::plugin::Operation<Value, Value>;
using Observe = std::function<Value(Context &)>;
const ValueOperation echo("example.host.v1.Echo/Call", harmos::VISIBILITY_PUBLIC);
const ValueOperation watching = ValueOperation("example.host.v1.Events/Watch", harmos::VISIBILITY_PUBLIC).watch();

/* === Classes === */

// Serves One Stream of Value Rows: a One-Sample or Window Observer, or a Device Publisher.
class SamplesHost final : public Instance::CallbackService {
public:
  vector<harmos::plugin::detail::StreamDefinition> streams;
  vector<harmos::plugin::detail::OperationDefinition> operations;
  map<string, string> requirements;
  size_t message_bytes = 4 * 1024 * 1024;
  unique_ptr<Server> server;
  shared_ptr<Channel> channel;
  mutex samples_mutex;
  condition_variable changed;
  size_t opened = 0;
  size_t closed = 0;

  explicit SamplesHost(Observe observer, bool device = false) {
    harmos::plugin::detail::StreamDefinition stream;
    stream.declaration.set_id("example.rows");
    stream.row = Value::descriptor();
    if (device)
      stream.producer = [observer](Context &context, auto) {
        observer(context);
      };
    else
      stream.sample = [observer](Context &context) -> unique_ptr<Message> {
        return std::make_unique<Value>(observer(context));
      };
    start(std::move(stream));
  }
  SamplesHost(std::function<Values(Context &, const harmos::plugin::Window &)> observer, size_t limit) : message_bytes(limit) {
    harmos::plugin::detail::StreamDefinition stream;
    stream.declaration.set_id("example.rows");
    stream.row = Value::descriptor();
    stream.window = [observer](Context &context, const harmos::plugin::Window &window) -> unique_ptr<Message> {
      return std::make_unique<Values>(observer(context, window));
    };
    start(std::move(stream));
  }
  ~SamplesHost() {
    server->Shutdown();
    server->Wait();
    unique_lock lock(samples_mutex);
    changed.wait(lock, [&] {
      return opened == closed;
    });
  }
  ServerBidiReactor<InstanceSamplesRequest, InstanceSamplesResponse> *Samples(CallbackServerContext *) override {
    auto *call = new harmos::plugin::detail::PublicationCall(streams, operations, requirements, "example", message_bytes);
    {
      lock_guard lock(samples_mutex);
      ++opened;
    }
    call->settled = [this] {
      lock_guard lock(samples_mutex);
      ++closed;
      changed.notify_all();
    };
    return call;
  }

private:
  void start(harmos::plugin::detail::StreamDefinition stream) {
    streams.push_back(std::move(stream));
    const auto declarations = harmos::plugin::Plugin("example", "1.0.0")
                                  .add([&](harmos::plugin::Registrar &ctx) {
                                    ctx.add(harmos::plugin::Dependency{echo});
                                  })
                                  .add([&](harmos::plugin::Registrar &ctx) {
                                    ctx.add(harmos::plugin::Dependency{watching});
                                  })
                                  .add([&](harmos::plugin::Registrar &ctx) {
                                    ctx.add(harmos::plugin::HostOperation::TimeGet);
                                  })
                                  .add([&](harmos::plugin::Registrar &ctx) {
                                    ctx.add(harmos::plugin::HostOperation::PluginsSettingsList);
                                  })
                                  .declarations();
    for (const auto &requirement : declarations.requires_())
      requirements[requirement.id()] = requirement.fingerprint();
    ServerBuilder builder;
    int port = 0;
    builder.AddListeningPort("127.0.0.1:0", InsecureServerCredentials(), &port);
    builder.RegisterService(this);
    server = builder.BuildAndStart();
    if (!server)
      throw runtime_error("Serve Samples: Bind Failed");
    channel = CreateChannel("127.0.0.1:" + std::to_string(port), InsecureChannelCredentials());
  }
};

class Samples {
public:
  ClientContext context;
  unique_ptr<ClientReaderWriter<InstanceSamplesRequest, InstanceSamplesResponse>> stream;
  bool finished = false;
  explicit Samples(const SamplesHost &host, bool open = true) : stream(Instance::NewStub(host.channel)->Samples(&context)) {
    if (open)
      this->open();
  }
  ~Samples() {
    if (!finished) {
      context.TryCancel();
      stream->Finish();
    }
  }
  void open() {
    InstanceSamplesRequest request;
    request.mutable_open()->set_stream("example.rows");
    request.mutable_open()->set_scope("opaque-scope");
    REQUIRE(stream->Write(request));
  }
  void window(int64_t seconds, uint64_t sequence, uint64_t count = 1) {
    InstanceSamplesRequest request;
    request.mutable_window()->mutable_start()->set_seconds(seconds);
    request.mutable_window()->mutable_period()->set_seconds(1);
    request.mutable_window()->set_count(count);
    request.mutable_window()->set_sequence(sequence);
    send(request);
  }
  void send(const InstanceSamplesRequest &request) {
    REQUIRE(stream->Write(request));
  }
  InstanceSamplesResponse read() {
    InstanceSamplesResponse response;
    REQUIRE(stream->Read(&response));
    return response;
  }
  // Reads Every Part of the Window and Returns Its Values in Sample Order.
  vector<string> answer(uint64_t count, size_t limit, size_t *parts = nullptr) {
    vector<string> values;
    for (;;) {
      const auto response = read();
      REQUIRE(response.samples().empty());
      REQUIRE(response.has_batch());
      const auto &batch = response.batch();
      REQUIRE(batch.offset() == values.size());
      REQUIRE(batch.count() >= 1);
      REQUIRE(response.ByteSizeLong() <= limit);
      REQUIRE(response.ByteSizeLong() ==
              harmos::plugin::detail::framed_size(batch.offset(), batch.count(), batch.columns().size(), batch.last()));
      const auto columns = harmos::plugin::decode<Values>(batch.columns());
      REQUIRE(static_cast<uint64_t>(columns.value_size()) == batch.count());
      values.insert(values.end(), columns.value().begin(), columns.value().end());
      if (parts)
        ++*parts;
      REQUIRE(batch.last() == (values.size() == count));
      if (batch.last())
        return values;
    }
  }
  void reply(uint64_t call, const Message &value) {
    InstanceSamplesRequest request;
    request.mutable_child()->set_call(call);
    request.mutable_child()->set_payload(harmos::plugin::detail::encode(value));
    REQUIRE(stream->Write(request));
  }
  Status finish() {
    stream->WritesDone();
    const auto result = stream->Finish();
    finished = true;
    return result;
  }
  Status refused() {
    InstanceSamplesResponse response;
    while (stream->Read(&response)) {
    }
    return finish();
  }
};

/* === Templates === */

/* === Public Functions === */

Value value(string bytes) {
  Value result;
  result.set_value(std::move(bytes));
  return result;
}

string little_endian(uint64_t number) {
  string bytes(8, '\0');
  for (unsigned index = 0; index < 8; ++index)
    bytes[index] = static_cast<char>(number >> (8 * index));
  return bytes;
}
} // namespace

TEST_CASE("Samples Correlate Reverse Replies Across Windows and Streams") {
  SamplesHost host([](Context &context) {
    auto first = context.call(echo, value("first"));
    auto second = context.call(echo, value("second"));
    return value(first.get().value() + second.get().value());
  });
  Samples first(host);
  Samples second(host);
  for (int64_t window = 1; window <= 2; ++window) {
    first.window(window, static_cast<uint64_t>(window));
    second.window(window, static_cast<uint64_t>(window));
    const auto one = first.read();
    const auto two = first.read();
    const auto other_one = second.read();
    const auto other_two = second.read();
    REQUIRE(one.samples().empty());
    REQUIRE(two.samples().empty());
    REQUIRE(one.child().call() == static_cast<uint64_t>(window * 2 - 1));
    REQUIRE(two.child().call() == static_cast<uint64_t>(window * 2));
    REQUIRE(other_one.child().call() == one.child().call());
    REQUIRE(other_two.child().call() == two.child().call());
    first.reply(two.child().call(), value("B"));
    second.reply(other_two.child().call(), value("Y"));
    second.reply(other_one.child().call(), value("X"));
    first.reply(one.child().call(), value("A"));
    REQUIRE(first.answer(1, host.message_bytes) == vector<string>{"AB"});
    REQUIRE(second.answer(1, host.message_bytes) == vector<string>{"XY"});
  }
  REQUIRE(first.finish().ok());
  REQUIRE(second.finish().ok());
}

TEST_CASE("A One-Sample Observer Answers Each Sample of a Window in Order") {
  SamplesHost host([](Context &context) {
    return value(little_endian(context.sequence));
  });
  Samples samples(host);
  samples.window(10, 5, 3);
  REQUIRE(samples.answer(3, host.message_bytes) == vector<string>{little_endian(5), little_endian(6), little_endian(7)});
  samples.window(13, 8, 2);
  REQUIRE(samples.answer(2, host.message_bytes) == vector<string>{little_endian(8), little_endian(9)});
  REQUIRE(samples.finish().ok());
}

TEST_CASE("Children Stay Open Across a Window's Samples Until It Is Answered") {
  SamplesHost host([](Context &context) {
    auto open = context.call(echo, value("open"));
    return context.call(echo, value("done")).get();
  });
  Samples samples(host);
  samples.window(1, 1, 2);
  vector<uint64_t> open;
  for (uint64_t sample = 0; sample < 2; ++sample) {
    const auto first = samples.read().child();
    const auto second = samples.read().child();
    REQUIRE(first.call() == sample * 2 + 1);
    REQUIRE(second.call() == sample * 2 + 2);
    REQUIRE(harmos::plugin::decode<Value>(first.payload()).value() == "open");
    open.push_back(first.call());
    samples.reply(second.call(), value(std::to_string(sample)));
  }
  REQUIRE(samples.read().child_cancel().call() == open[0]);
  REQUIRE(samples.read().child_cancel().call() == open[1]);
  REQUIRE(samples.answer(2, host.message_bytes) == vector<string>{"0", "1"});
  REQUIRE(samples.finish().ok());
}

TEST_CASE("A Window Observer Answers the Whole Grant in Parts Within the Message Limit") {
  const size_t limit = 64;
  SamplesHost host(
      [](Context &context, const harmos::plugin::Window &window) {
        if (context.sequence != window.sequence || window.start.seconds() != 20 || window.period.seconds() != 1)
          throw runtime_error("Answer Window: Unexpected Grant");
        Values columns;
        for (uint64_t index = 0; index < window.count; ++index)
          columns.add_value(little_endian(window.sequence + index));
        return columns;
      },
      limit);
  Samples samples(host);
  samples.window(20, 3, 25);
  size_t parts = 0;
  const auto values = samples.answer(25, limit, &parts);
  REQUIRE(parts > 1);
  for (uint64_t index = 0; index < values.size(); ++index)
    REQUIRE(values[index] == little_endian(3 + index));
  REQUIRE(samples.finish().ok());
}

TEST_CASE("A Window That Cannot Be Answered Fails the Call") {
  SECTION("A Sample Larger Than the Message Limit Fails the Window") {
    SamplesHost host(
        [](Context &, const harmos::plugin::Window &window) {
          Values columns;
          for (uint64_t index = 0; index < window.count; ++index)
            columns.add_value(string(index == 1 ? 100 : 1, 'x'));
          return columns;
        },
        64);
    Samples samples(host);
    samples.window(1, 1, 3);
    REQUIRE(samples.refused().error_code() == StatusCode::RESOURCE_EXHAUSTED);
  }
  SECTION("Columns Must Hold One Value per Sample") {
    SamplesHost host(
        [](Context &, const harmos::plugin::Window &) {
          return Values{};
        },
        64);
    Samples samples(host);
    samples.window(1, 1, 2);
    REQUIRE(samples.refused().error_code() == StatusCode::INTERNAL);
  }
}

TEST_CASE("Samples Refuse Invalid Windows") {
  SamplesHost host([](Context &) {
    return Value{};
  });
  SECTION("A Window Before Open") {
    Samples samples(host, false);
    samples.window(1, 1);
    REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
  }
  SECTION("A Repeated Open") {
    Samples samples(host);
    samples.open();
    REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
  }
  SECTION("A Missing Start") {
    Samples samples(host);
    InstanceSamplesRequest request;
    request.mutable_window()->mutable_period()->set_seconds(1);
    request.mutable_window()->set_count(1);
    samples.send(request);
    REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
  }
  SECTION("A Start Outside the Timestamp Range") {
    Samples samples(host);
    InstanceSamplesRequest request;
    request.mutable_window()->mutable_start()->set_seconds(253402300800LL);
    request.mutable_window()->mutable_period()->set_seconds(1);
    request.mutable_window()->set_count(1);
    samples.send(request);
    REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
  }
  SECTION("A Missing Period") {
    Samples samples(host);
    InstanceSamplesRequest request;
    request.mutable_window()->mutable_start()->set_seconds(1);
    request.mutable_window()->set_count(1);
    samples.send(request);
    REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
  }
  SECTION("A Non-Positive Period") {
    Samples samples(host);
    InstanceSamplesRequest request;
    request.mutable_window()->mutable_start()->set_seconds(1);
    request.mutable_window()->mutable_period()->set_seconds(-1);
    request.mutable_window()->set_count(1);
    samples.send(request);
    REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
  }
  SECTION("A Zero Count") {
    Samples samples(host);
    samples.window(1, 1, 0);
    REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
  }
}

TEST_CASE("Samples Refuse a Window While Another Is Unanswered") {
  SamplesHost host([](Context &context) {
    return context.call(echo, Value{}).get();
  });
  Samples samples(host);
  samples.window(1, 1);
  REQUIRE(samples.read().has_child());
  samples.window(2, 2);
  REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
}

TEST_CASE("Samples Refuse a Window on a Device Stream") {
  promise<void> published;
  auto ready = published.get_future();
  SamplesHost host(
      [&published](Context &context) {
        published.set_value();
        context.cancelled();
        return Value{};
      },
      true);
  Samples samples(host);
  ready.get();
  samples.window(1, 1);
  REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
}

TEST_CASE("A Window Calls the Clock Settings and a Demand-Bounded Watch") {
  SamplesHost host([](Context &context) {
    const auto now = context.now();
    const auto settings = context.settings();
    auto watch = context.watch(watching, value(context.scope()), 4096);
    string result = std::to_string(now.seconds()) + settings.value().values().at("label").text();
    result += settings.declarations(0).name() + settings.revision();
    result += std::to_string(settings.resolved_at().positions(0).sequence());
    while (auto item = watch.next())
      result += item->value();
    return value(result);
  });
  Samples samples(host);
  samples.window(10, 1);
  auto child = samples.read();
  REQUIRE(child.child().operation() == "harmos.host.v1.Time/Get");
  harmos::host::v1::TimeGetResponse now;
  now.mutable_now()->set_seconds(42);
  samples.reply(child.child().call(), now);
  child = samples.read();
  REQUIRE(child.child().operation() == "harmos.host.v1.plugins.Settings/List");
  const auto request = harmos::plugin::decode<harmos::host::v1::plugins::SettingsListRequest>(child.child().payload());
  REQUIRE_FALSE(request.has_name());
  harmos::host::v1::plugins::SettingsListResponse settings;
  (*settings.mutable_value()->mutable_values())["label"].set_text("configured");
  settings.add_declarations()->set_name("label");
  settings.set_revision("revision");
  settings.mutable_resolved_at()->add_positions()->set_sequence(17);
  samples.reply(child.child().call(), settings);
  child = samples.read();
  REQUIRE(child.child().watch());
  REQUIRE(child.child().max_response_bytes() == 4096);
  REQUIRE(harmos::plugin::decode<Value>(child.child().payload()).value() == "opaque-scope");
  const auto call = child.child().call();
  REQUIRE(samples.read().child_demand().call() == call);
  InstanceSamplesRequest item;
  item.mutable_child()->set_call(call);
  item.mutable_child()->set_item(harmos::plugin::detail::encode(value("item")));
  REQUIRE(samples.stream->Write(item));
  REQUIRE(samples.read().child_demand().call() == call);
  samples.reply(call, Value{});
  REQUIRE(samples.answer(1, host.message_bytes) == vector<string>{"42configuredlabelrevision17item"});
  REQUIRE(samples.finish().ok());
}

TEST_CASE("A Window Preserves Every Structured Child Failure Field") {
  SamplesHost host([](Context &context) {
    try {
      context.call(echo, Value{}).get();
    } catch (const harmos::Failure &failure) {
      return value(harmos::plugin::detail::encode(failure.value));
    }
    throw runtime_error("Call Echo: Expected Failure");
  });
  Samples samples(host);
  samples.window(1, 1);
  const auto child = samples.read();
  harmos::FailureValue failure;
  failure.set_classification(harmos::CLASSIFICATION_REFUSED);
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
  const auto actual = harmos::plugin::decode<harmos::FailureValue>(samples.answer(1, host.message_bytes).at(0));
  REQUIRE(harmos::plugin::detail::encode(actual) == harmos::plugin::detail::encode(failure));
  REQUIRE(samples.finish().ok());
}

TEST_CASE("Answering a Window Cancels Children and Rejects Late Replies") {
  SamplesHost host([](Context &context) {
    auto cancelled = context.call(echo, Value{});
    cancelled.cancel();
    context.call(echo, Value{});
    return Value{};
  });
  Samples samples(host);
  samples.window(1, 1);
  const auto first = samples.read().child().call();
  REQUIRE(samples.read().child_cancel().call() == first);
  const auto second = samples.read().child().call();
  REQUIRE(second != first);
  REQUIRE(samples.read().child_cancel().call() == second);
  REQUIRE(samples.read().batch().last());
  samples.reply(second, Value{});
  REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
}

TEST_CASE("Samples Refuse Outside-Window and Duplicate Child Traffic") {
  SECTION("A Reply Before a Window Is Refused") {
    SamplesHost host([](Context &) {
      return Value{};
    });
    Samples samples(host);
    samples.reply(1, Value{});
    REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
  }
  SECTION("A Publisher Cannot Call Host Operations") {
    SamplesHost host(
        [](Context &context) {
          return context.call(echo, Value{}).get();
        },
        true);
    Samples samples(host);
    REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
  }
  SECTION("A Duplicate Reply Cannot Satisfy Another Child") {
    SamplesHost host([](Context &context) {
      context.call(echo, Value{}).get();
      return context.call(echo, Value{}).get();
    });
    Samples samples(host);
    samples.window(1, 1);
    const auto call = samples.read().child().call();
    samples.reply(call, Value{});
    REQUIRE(samples.read().child().call() != call);
    samples.reply(call, Value{});
    REQUIRE(samples.refused().error_code() == StatusCode::INVALID_ARGUMENT);
  }
}

TEST_CASE("Ending Samples Settles Open Child Calls") {
  promise<void> settled;
  auto done = settled.get_future();
  SamplesHost host([&settled](Context &context) {
    try {
      context.call(echo, Value{}).get();
    } catch (const harmos::TransportError &) {
      settled.set_value();
      throw;
    }
    return Value{};
  });
  Samples samples(host);
  samples.window(1, 1);
  REQUIRE(samples.read().has_child());
  REQUIRE(samples.finish().ok());
  done.get();
}

/* === Private Functions === */

/* === Entry Point === */
