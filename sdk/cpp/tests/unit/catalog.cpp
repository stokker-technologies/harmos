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
#include "harmos/_generated/client.hpp"
#include "harmos/harmos.hpp"
#include "harmos/host/v1/time.pb.h"

namespace authoring = harmos::plugin;
namespace host_wire = harmos::host::v1;
namespace sdk = harmos;
using google::protobuf::DescriptorPool;
using google::protobuf::Duration;
using google::protobuf::SimpleDescriptorDatabase;
using std::invalid_argument;
using std::string;

/* === Macros === */

/* === Constants === */

/* === Types === */

using Value = harmos::conformance::Value;

/* === Classes === */

/* === Templates === */

template <class T>
concept ReadsTime = requires(T root) { root.time.get(); };
template <class T>
concept CreatesInvocation = requires(T root) { root.invocations.create(string{}, string{}, string{}); };
template <class T>
concept ListsSettings = requires(T root) { root.plugins.settings.list(); };
template <class T>
concept SetsSettings = requires(T root) { root.plugins.settings.set(string{}, sdk::SettingValues{}, string{}); };

/* === Public Functions === */

TEST_CASE("Caller Options Select SDK Surfaces and Composition Narrows Them") {
  REQUIRE(ReadsTime<sdk::Client>);
  REQUIRE(ReadsTime<authoring::HostRoot>);
  REQUIRE_FALSE(ReadsTime<fixture::Client>);
  REQUIRE(CreatesInvocation<sdk::Client>);
  REQUIRE_FALSE(CreatesInvocation<authoring::HostRoot>);
  REQUIRE(ListsSettings<sdk::Client>);
  REQUIRE(ListsSettings<authoring::HostRoot>);
  REQUIRE(SetsSettings<sdk::Client>);
  REQUIRE_FALSE(SetsSettings<authoring::HostRoot>);
}

TEST_CASE("A declarations refuses duplicate operations") {
  SECTION("Duplicate Registrations Are Refused") {
    authoring::Operation<Value, Value> echo("example.echo", sdk::VISIBILITY_PUBLIC, [](authoring::Context &, Value request) {
      return request;
    });
    authoring::Plugin plugin("example", "1.0.0");
    plugin.operation(echo).operation(echo);
    REQUIRE_THROWS_AS(plugin.declarations(), invalid_argument);
  }
}

TEST_CASE("An observer preserves its stream contract") {
  SECTION("Row Types Stay Stable and Incompatible Rows Are Refused") {
    authoring::Stream<Value> stream("example.samples");
    authoring::Observer<Value> observer(stream, [](authoring::Context &) {
      return Value{};
    });
    Duration interval;
    interval.set_seconds(2);
    observer.every(interval);
    authoring::Plugin plugin("example", "1.0.0");
    plugin.stream(stream).observer(observer);
    auto declarations = plugin.declarations();
    REQUIRE(declarations.streams(0).row() == Value::descriptor()->full_name());

    authoring::Stream<host_wire::TimeGetRequest> different_row("example.samples");
    authoring::Observer<host_wire::TimeGetRequest> incompatible(different_row, [](authoring::Context &) {
      return host_wire::TimeGetRequest{};
    });
    incompatible.every(interval);
    authoring::Plugin invalid("example", "1.0.0");
    invalid.stream(stream);
    REQUIRE_THROWS_AS(invalid.observer(incompatible), invalid_argument);
  }
}

TEST_CASE("A declarations preserves typed sections and their dependencies") {
  SECTION("Typed Sections Retain Their Values and Resolve Dependencies") {
    host_wire::TimeSleepRequest setting;
    setting.mutable_duration()->set_seconds(37);
    authoring::Plugin plugin("example", "1.0.0");
    plugin.requires_operation(authoring::HostOperation::TimeGet).requires_operation(authoring::HostOperation::TimeSleep).section(setting);
    setting.mutable_duration()->set_seconds(99);
    const auto declarations = plugin.declarations();
    const auto decoded = authoring::decode<host_wire::TimeSleepRequest>(declarations.sections(0).payload());
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

TEST_CASE("Settings Preserve Typed Defaults and Refuse Duplicate Names") {
  sdk::SettingDeclaration flag;
  flag.set_name("enabled");
  flag.mutable_default_()->set_flag(false);
  sdk::SettingDeclaration interval;
  interval.set_name("interval");
  interval.mutable_default_()->mutable_duration()->set_seconds(2);
  authoring::Plugin plugin("example", "1.0.0");
  plugin.setting(flag).setting(interval).requires_operation(authoring::HostOperation::PluginsSettingsList);
  const auto declarations = plugin.declarations();
  const auto settings = authoring::decode<sdk::SettingsDeclarations>(declarations.sections(0).payload());
  REQUIRE(settings.values(0).default_().has_flag());
  REQUIRE_FALSE(settings.values(0).default_().flag());
  REQUIRE(settings.values(1).default_().duration().seconds() == 2);
  REQUIRE_THROWS_AS(plugin.setting(flag), invalid_argument);
  interval.clear_default_();
  REQUIRE_THROWS_AS(authoring::Plugin("invalid", "1.0.0").setting(interval), invalid_argument);
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
