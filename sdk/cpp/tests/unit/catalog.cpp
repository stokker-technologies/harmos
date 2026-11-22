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
#include <catch2/catch_session.hpp>
#include <catch2/catch_test_macros.hpp>
#include <google/protobuf/descriptor_database.h>
#include <grpc/grpc.h>

// Local Imports
#include "client.hpp"
#include "generated/types.hpp"
#include "harmos/harmos.hpp"
#include "harmos/host/v1/time.pb.h"
#include "plugin.hpp"

using google::protobuf::DescriptorPool;
using google::protobuf::Duration;
using google::protobuf::SimpleDescriptorDatabase;
using std::invalid_argument;
using std::string;

/* === Macros === */

/* === Constants === */

/* === Types === */

using harmos::conformance::Value;

/* === Classes === */

/* === Templates === */

template <class T>
concept ReadsTime = requires(T root) { root.time.get(); };
template <class T>
concept Sleeps = requires(T root) { root.time.sleep(Duration{}); };
template <class T>
concept CreatesInvocation = requires(T root) { root.invocations.create(string{}, string{}, string{}); };
template <class T>
concept ListsSettings = requires(T root) { root.plugins.settings.list(); };
template <class T>
concept SetsSettings = requires(T root) { root.plugins.settings.set(string{}, harmos::SettingValues{}, string{}); };

/* === Public Functions === */

TEST_CASE("Caller Options Select SDK Surfaces") {
  REQUIRE(ReadsTime<fixture::Client>);
  REQUIRE(ReadsTime<harmos::plugin::HostRoot>);
  STATIC_REQUIRE(ReadsTime<fixture::plugin::HostRoot>);
  STATIC_REQUIRE(Sleeps<fixture::plugin::HostRoot>);
  REQUIRE(CreatesInvocation<fixture::Client>);
  REQUIRE_FALSE(CreatesInvocation<harmos::plugin::HostRoot>);
  REQUIRE(ListsSettings<fixture::Client>);
  REQUIRE(ListsSettings<harmos::plugin::HostRoot>);
  REQUIRE(SetsSettings<fixture::Client>);
  REQUIRE_FALSE(SetsSettings<harmos::plugin::HostRoot>);
}

TEST_CASE("Lanes Encode Facts and Refuse Invalid Declarations") {
  const auto instrument = harmos::plugin::Lane::serial("instrument");
  const auto parallel = harmos::plugin::Lane::concurrent("parallel", 2);
  const auto open = harmos::plugin::Lane::unbounded("open");
  harmos::plugin::Operation<Value, Value> operation("example.measure", harmos::VISIBILITY_PUBLIC,
                                                    [](harmos::plugin::Context &, Value request) {
                                                      return request;
                                                    });
  operation.lane(instrument);
  harmos::plugin::Plugin plugin("example", "1.0.0");
  plugin.lane(instrument).lane(parallel).lane(open).add([&](harmos::plugin::Registrar &ctx) {
    ctx.add(operation);
  });
  const auto declarations = plugin.declarations();
  REQUIRE(declarations.lanes(0).SerializeAsString() == string("\x0a\x0a"
                                                              "instrument"
                                                              "\x10\x01",
                                                              14));
  REQUIRE(declarations.lanes(1).SerializeAsString() == string("\x0a\x08"
                                                              "parallel"
                                                              "\x18\x02",
                                                              12));
  REQUIRE(declarations.lanes(2).SerializeAsString() == string("\x0a\x04"
                                                              "open"
                                                              "\x20\x01",
                                                              8));
  REQUIRE(declarations.operations(0).lane() == "instrument");
  REQUIRE_THROWS_AS(harmos::plugin::Lane::serial(""), invalid_argument);
  REQUIRE_THROWS_AS(harmos::plugin::Lane::concurrent("instrument", 0), invalid_argument);
  REQUIRE_THROWS_AS(plugin.lane(instrument), invalid_argument);
  harmos::plugin::Plugin undeclared("example", "1.0.0");
  undeclared.add([&](harmos::plugin::Registrar &ctx) {
    ctx.add(operation);
  });
  REQUIRE_THROWS_AS(undeclared.declarations(), invalid_argument);
}

TEST_CASE("A Declaration Refuses Duplicate Operations") {
  SECTION("Duplicate Registrations Are Refused") {
    harmos::plugin::Operation<Value, Value> echo("example.echo", harmos::VISIBILITY_PUBLIC, [](harmos::plugin::Context &, Value request) {
      return request;
    });
    harmos::plugin::Plugin plugin("example", "1.0.0");
    plugin
        .add([&](harmos::plugin::Registrar &ctx) {
          ctx.add(echo);
        })
        .add([&](harmos::plugin::Registrar &ctx) {
          ctx.add(echo);
        });
    REQUIRE_THROWS_AS(plugin.declarations(), invalid_argument);
  }
}

TEST_CASE("An Observer Preserves Its Stream Contract") {
  SECTION("Row Types Stay Stable and Incompatible Rows Are Refused") {
    harmos::plugin::Stream<Value> stream("example.samples");
    harmos::plugin::Observer<Value> observer(stream, [](harmos::plugin::Context &) {
      return Value{};
    });
    Duration interval;
    interval.set_seconds(2);
    observer.every(interval);
    harmos::plugin::Plugin plugin("example", "1.0.0");
    plugin
        .add([&](harmos::plugin::Registrar &ctx) {
          ctx.add(stream);
        })
        .add([&](harmos::plugin::Registrar &ctx) {
          ctx.add(observer);
        });
    auto declarations = plugin.declarations();
    REQUIRE(declarations.streams(0).row() == Value::descriptor()->full_name());

    harmos::plugin::Stream<harmos::host::v1::TimeGetRequest> different_row("example.samples");
    harmos::plugin::Observer<harmos::host::v1::TimeGetRequest> incompatible(different_row, [](harmos::plugin::Context &) {
      return harmos::host::v1::TimeGetRequest{};
    });
    incompatible.every(interval);
    harmos::plugin::Plugin invalid("example", "1.0.0");
    invalid.add([&](harmos::plugin::Registrar &ctx) {
      ctx.add(stream);
    });
    REQUIRE_THROWS_AS(invalid.add([&](harmos::plugin::Registrar &ctx) {
      ctx.add(incompatible);
    }),
                      invalid_argument);
  }
}

TEST_CASE("Interval Observers Declare One Value per Field and Their Window Columns") {
  using ::conformance::model::v1::Values;
  using harmos::conformance::Graph;
  Duration interval;
  interval.set_seconds(1);
  SECTION("Repeated, Map, Oneof and Optional Row Fields Are Refused") {
    harmos::plugin::Stream<Graph> repeated("example.graph");
    REQUIRE_THROWS_AS(harmos::plugin::Observer<Graph>(repeated,
                                                      [](harmos::plugin::Context &) {
                                                        return Graph{};
                                                      }),
                      invalid_argument);
    harmos::plugin::Stream<Graph::Nested> optional("example.nested");
    REQUIRE_THROWS_AS(harmos::plugin::Observer<Graph::Nested>(optional,
                                                              [](harmos::plugin::Context &) {
                                                                return Graph::Nested{};
                                                              }),
                      invalid_argument);
  }
  SECTION("Window Columns Must Repeat Each Row Field") {
    harmos::plugin::Stream<Value> stream("example.samples");
    const auto mismatched = [](harmos::plugin::Context &, const harmos::plugin::Window &) {
      return Value{};
    };
    REQUIRE_THROWS_AS(harmos::plugin::Observer<Value>::window<Value>(stream, mismatched), invalid_argument);
  }
  SECTION("A Window Observer Declares Its Window Setting and Columns Type") {
    harmos::plugin::Stream<Value> stream("example.samples");
    auto observer = harmos::plugin::Observer<Value>::window<Values>(stream, [](harmos::plugin::Context &, const harmos::plugin::Window &) {
      return Values{};
    });
    observer.every(interval).window_setting("window");
    harmos::plugin::Plugin plugin("example", "1.0.0");
    plugin
        .add([&](harmos::plugin::Registrar &ctx) {
          ctx.add(stream);
        })
        .add([&](harmos::plugin::Registrar &ctx) {
          ctx.add(observer);
        });
    const auto declarations = plugin.declarations();
    REQUIRE(declarations.streams(0).window_setting() == "window");
    REQUIRE(declarations.streams(0).row() == Value::descriptor()->full_name());
    bool columns = false;
    for (const auto &file : declarations.types().file())
      for (const auto &message : file.message_type())
        columns = columns || (file.package() + "." + message.name()) == Values::descriptor()->full_name();
    REQUIRE(columns);
  }
}

TEST_CASE("A Declaration Preserves Typed Sections and Their Dependencies") {
  SECTION("Typed Sections Retain Their Values and Resolve Dependencies") {
    harmos::host::v1::TimeSleepRequest setting;
    setting.mutable_duration()->set_seconds(37);
    harmos::plugin::Plugin plugin("example", "1.0.0");
    plugin
        .add([&](harmos::plugin::Registrar &ctx) {
          ctx.add(harmos::plugin::HostOperation::TimeGet);
        })
        .add([&](harmos::plugin::Registrar &ctx) {
          ctx.add(harmos::plugin::HostOperation::TimeSleep);
        })
        .add([&](harmos::plugin::Registrar &ctx) {
          ctx.add(harmos::plugin::Contribution{setting});
        });
    setting.mutable_duration()->set_seconds(99);
    const auto declarations = plugin.declarations();
    const auto decoded = harmos::plugin::decode<harmos::host::v1::TimeSleepRequest>(declarations.sections(0).payload());
    REQUIRE(decoded.duration().seconds() == 37);

    SimpleDescriptorDatabase database;
    for (const auto &file : declarations.types().file()) {
      REQUIRE(database.Add(file));
    }
    DescriptorPool pool(&database);
    for (const auto *name : {"harmos.host.v1.TimeSleepRequest", "harmos.host.v1.TimeSleepResponse", "harmos.host.v1.TimeGetRequest",
                             "harmos.host.v1.TimeGetResponse"}) {
      REQUIRE(pool.FindMessageTypeByName(name));
    }
    REQUIRE(pool.FindMessageTypeByName(declarations.sections(0).type()));
  }
}

TEST_CASE("Settings Preserve Typed Defaults") {
  harmos::SettingDeclaration flag;
  flag.set_name("enabled");
  flag.mutable_default_()->set_flag(false);
  harmos::SettingDeclaration interval;
  interval.set_name("interval");
  interval.mutable_default_()->mutable_duration()->set_seconds(2);
  interval.mutable_minimum()->mutable_duration()->set_seconds(1);
  interval.mutable_maximum()->mutable_duration()->set_seconds(3);
  harmos::plugin::Plugin plugin("example", "1.0.0");
  plugin
      .add([&](harmos::plugin::Registrar &ctx) {
        ctx.add(flag);
      })
      .add([&](harmos::plugin::Registrar &ctx) {
        ctx.add(interval);
      })
      .add([&](harmos::plugin::Registrar &ctx) {
        ctx.add(harmos::plugin::HostOperation::PluginsSettingsList);
      });
  const auto declarations = plugin.declarations();
  const auto settings = harmos::plugin::decode<harmos::SettingsDeclarations>(declarations.sections(0).payload());
  REQUIRE(settings.values(0).default_().has_flag());
  REQUIRE_FALSE(settings.values(0).default_().flag());
  REQUIRE(settings.values(1).default_().duration().seconds() == 2);
  REQUIRE(settings.values(1).minimum().duration().seconds() == 1);
  REQUIRE(settings.values(1).maximum().duration().seconds() == 3);
  SimpleDescriptorDatabase database;
  for (const auto &file : declarations.types().file())
    REQUIRE(database.Add(file));
  DescriptorPool pool(&database);
  REQUIRE(pool.FindMessageTypeByName("harmos.model.v1.SettingValue"));
  REQUIRE(pool.FindMessageTypeByName("harmos.host.v1.plugins.SettingsListResponse"));
}

/* === Private Functions === */

/* === Entry Point === */

int main(int argc, char **argv) {
  grpc_init();
  const auto result = Catch::Session{}.run(argc, argv);
  grpc_shutdown_blocking();
  return result;
}
