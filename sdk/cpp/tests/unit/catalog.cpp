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
#include <google/protobuf/descriptor_database.h>

// Local Imports
#include "harmos/harmos.hpp"
#include "harmos/generated/client.hpp"
#include "harmos/context/v1/time.pb.h"
#include "harmos/conformance/v1/fixture.pb.h"

/* === Macros === */

/* === Constants === */

/* === Types === */

using Value = harmos::conformance::v1::Value;

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

TEST_CASE("A catalog refuses duplicate operations") {
    SECTION("A catalog refuses duplicate operations") {
        harmos::Operation<Value, Value> echo("example.echo", harmos::wire::VISIBILITY_PUBLIC, [](harmos::Context&, Value request) { return request; });
        harmos::Plugin plugin("example", "1.0.0");
        plugin.operation(echo).operation(echo);
        REQUIRE_THROWS_AS(plugin.catalog(), std::invalid_argument);
    }
}

TEST_CASE("An observer preserves its stream contract") {
    SECTION("An observer preserves its stream contract") {
        harmos::Stream<Value> stream("example.samples");
        harmos::Observer<Value> observer(stream, [](harmos::Context&) { return Value{}; });
        google::protobuf::Duration interval;
        interval.set_seconds(2);
        observer.every(interval);
        harmos::Plugin plugin("example", "1.0.0");
        plugin.stream(stream).observer(observer);
        auto catalog = plugin.catalog();
        REQUIRE(catalog.streams(0).row() == Value::descriptor()->full_name());
    
        harmos::Stream<harmos::context::v1::TimeGetRequest> different_row("example.samples");
        harmos::Observer<harmos::context::v1::TimeGetRequest> incompatible(different_row, [](harmos::Context&) { return harmos::context::v1::TimeGetRequest{}; });
        incompatible.every(interval);
        harmos::Plugin invalid("example", "1.0.0");
        invalid.stream(stream);
        REQUIRE_THROWS_AS(invalid.observer(incompatible), std::invalid_argument);
    }
}

TEST_CASE("A catalog preserves typed sections and their dependencies") {
    SECTION("A catalog preserves typed sections and their dependencies") {
        harmos::context::v1::TimeSleepRequest setting;
        setting.mutable_duration()->set_seconds(37);
        harmos::Plugin plugin("example", "1.0.0");
        plugin.requires_operation(harmos::GET_TIME).requires_operation(harmos::SLEEP).section(setting);
        setting.mutable_duration()->set_seconds(99);
        const auto catalog = plugin.catalog();
        const auto decoded = harmos::decode<harmos::context::v1::TimeSleepRequest>(catalog.sections(0).payload());
        REQUIRE(decoded.duration().seconds() == 37);
    
        google::protobuf::SimpleDescriptorDatabase database;
        for (const auto& file : catalog.types().file()) {
            REQUIRE(database.Add(file));
        }
        google::protobuf::DescriptorPool pool(&database);
        for (const auto* name : {"harmos.context.v1.TimeSleepRequest", "harmos.context.v1.TimeSleepResponse", "harmos.context.v1.TimeGetRequest", "harmos.context.v1.TimeGetResponse"}) {
            REQUIRE(pool.FindMessageTypeByName(name));
        }
        REQUIRE(pool.FindMessageTypeByName(catalog.sections(0).type()));
    }
}

/* === Private Functions === */

/* === Entry Point === */

