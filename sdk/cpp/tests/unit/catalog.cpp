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
#include <catch2/catch_test_macros.hpp>
#include <catch2/catch_session.hpp>
#include <grpc/grpc.h>
#include <google/protobuf/descriptor_database.h>

// Local Imports
#include "harmos/harmos.hpp"
#include "harmos/generated/client.hpp"
#include "harmos/plugin/host/v1/time.pb.h"
#include "harmos/conformance/v1/fixture.pb.h"

/* === Macros === */

/* === Constants === */

/* === Types === */

using Value = harmos::conformance::v1::Value;

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

TEST_CASE("A declarations refuses duplicate operations") {
    SECTION("Duplicate Registrations Are Refused") {
        harmos::Operation<Value, Value> echo("example.echo", harmos::wire::VISIBILITY_PUBLIC, [](harmos::Context&, Value request) { return request; });
        harmos::Plugin plugin("example", "1.0.0");
        plugin.operation(echo).operation(echo);
        REQUIRE_THROWS_AS(plugin.declarations(), std::invalid_argument);
    }
}

TEST_CASE("An observer preserves its stream contract") {
    SECTION("Row Types Stay Stable and Incompatible Rows Are Refused") {
        harmos::Stream<Value> stream("example.samples");
        harmos::Observer<Value> observer(stream, [](harmos::Context&) { return Value{}; });
        google::protobuf::Duration interval;
        interval.set_seconds(2);
        observer.every(interval);
        harmos::Plugin plugin("example", "1.0.0");
        plugin.stream(stream).observer(observer);
        auto declarations = plugin.declarations();
        REQUIRE(declarations.streams(0).row() == Value::descriptor()->full_name());

        harmos::Stream<harmos::plugin::host::v1::TimeGetRequest> different_row("example.samples");
        harmos::Observer<harmos::plugin::host::v1::TimeGetRequest> incompatible(different_row, [](harmos::Context&) { return harmos::plugin::host::v1::TimeGetRequest{}; });
        incompatible.every(interval);
        harmos::Plugin invalid("example", "1.0.0");
        invalid.stream(stream);
        REQUIRE_THROWS_AS(invalid.observer(incompatible), std::invalid_argument);
    }
}

TEST_CASE("A declarations preserves typed sections and their dependencies") {
    SECTION("Typed Sections Retain Their Values and Resolve Dependencies") {
        harmos::plugin::host::v1::TimeSleepRequest setting;
        setting.mutable_duration()->set_seconds(37);
        harmos::Plugin plugin("example", "1.0.0");
        plugin.requires_operation(harmos::GET_TIME).requires_operation(harmos::SLEEP).section(setting);
        setting.mutable_duration()->set_seconds(99);
        const auto declarations = plugin.declarations();
        const auto decoded = harmos::decode<harmos::plugin::host::v1::TimeSleepRequest>(declarations.sections(0).payload());
        REQUIRE(decoded.duration().seconds() == 37);

        google::protobuf::SimpleDescriptorDatabase database;
        for (const auto& file : declarations.types().file()) {
            REQUIRE(database.Add(file));
        }
        google::protobuf::DescriptorPool pool(&database);
        for (const auto* name : {"harmos.plugin.host.v1.TimeSleepRequest", "harmos.plugin.host.v1.TimeSleepResponse", "harmos.plugin.host.v1.TimeGetRequest", "harmos.plugin.host.v1.TimeGetResponse"}) {
            REQUIRE(pool.FindMessageTypeByName(name));
        }
        REQUIRE(pool.FindMessageTypeByName(declarations.sections(0).type()));
    }
}

TEST_CASE("Settings Preserve Typed Defaults and Refuse Duplicate Names") {
    harmos::wire::SettingDeclaration flag;
    flag.set_name("enabled");
    flag.mutable_default_()->set_flag(false);
    harmos::wire::SettingDeclaration interval;
    interval.set_name("interval");
    interval.mutable_default_()->mutable_duration()->set_seconds(2);
    harmos::Plugin plugin("example", "1.0.0");
    plugin.setting(flag).setting(interval).requires_operation(harmos::LIST_SETTINGS);
    const auto declarations = plugin.declarations();
    const auto settings = harmos::decode<harmos::wire::SettingsDeclarations>(declarations.sections(0).payload());
    REQUIRE(settings.values(0).default_().has_flag());
    REQUIRE_FALSE(settings.values(0).default_().flag());
    REQUIRE(settings.values(1).default_().duration().seconds() == 2);
    REQUIRE_THROWS_AS(plugin.setting(flag), std::invalid_argument);
    interval.clear_default_();
    REQUIRE_THROWS_AS(harmos::Plugin("invalid", "1.0.0").setting(interval), std::invalid_argument);
    google::protobuf::SimpleDescriptorDatabase database;
    for (const auto& file : declarations.types().file()) REQUIRE(database.Add(file));
    google::protobuf::DescriptorPool pool(&database);
    REQUIRE(pool.FindMessageTypeByName("harmos.v1.SettingValue"));
    REQUIRE(pool.FindMessageTypeByName("harmos.plugin.host.v1.SettingsListResponse"));
}

/* === Private Functions === */

/* === Entry Point === */

int main(int argc, char** argv) {
    grpc_init();
    const auto result = Catch::Session{}.run(argc, argv);
    grpc_shutdown_blocking();
    return result;
}

