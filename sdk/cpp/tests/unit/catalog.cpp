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
#include <google/protobuf/descriptor_database.h>

// Local Imports
#include "harmos/harmos.hpp"
#include "harmos/context/v1/time.pb.h"
#include "harmos/conformance/v1/fixture.pb.h"

/* === Macros === */

/* === Constants === */

/* === Types === */

using Value = harmos::conformance::v1::Value;

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

void catalog_rejects_duplicate_operations() {
    harmos::Operation<Value, Value> echo("example.echo", [](harmos::Context&, Value request) { return request; });
    harmos::Plugin plugin("example", "1.0.0");
    plugin.operation(echo).operation(echo);
    try { plugin.catalog(); }
    catch (const std::invalid_argument&) { return; }
    throw std::logic_error("Duplicate Operation Was Admitted");
}

void observer_preserves_its_stream_contract() {
    harmos::Stream<Value> stream("example.samples");
    harmos::Observer<Value> observer(stream, [](harmos::Context&) { return Value{}; });
    google::protobuf::Duration interval;
    interval.set_seconds(2);
    observer.every(interval);
    harmos::Plugin plugin("example", "1.0.0");
    plugin.stream(stream).observer(observer);
    auto catalog = plugin.catalog();
    if (catalog.streams(0).row() != Value::descriptor()->full_name()) throw std::logic_error("Observer Replaced the Stream Row Type");

    harmos::Stream<harmos::context::v1::TimeGetRequest> different_row("example.samples");
    harmos::Observer<harmos::context::v1::TimeGetRequest> incompatible(different_row, [](harmos::Context&) { return harmos::context::v1::TimeGetRequest{}; });
    incompatible.every(interval);
    harmos::Plugin invalid("example", "1.0.0");
    invalid.stream(stream);
    try { invalid.observer(incompatible); }
    catch (const std::invalid_argument&) { return; }
    throw std::logic_error("Incompatible Observer Row Was Admitted");
}

void catalog_preserves_typed_sections_and_dependencies() {
    harmos::context::v1::TimeSleepRequest setting;
    setting.mutable_duration()->set_seconds(37);
    harmos::Plugin plugin("example", "1.0.0");
    plugin.requires_operation(harmos::GET_TIME).requires_operation(harmos::SLEEP).section(setting);
    setting.mutable_duration()->set_seconds(99);
    const auto catalog = plugin.catalog();
    const auto decoded = harmos::decode<harmos::context::v1::TimeSleepRequest>(catalog.sections(0).payload());
    if (decoded.duration().seconds() != 37) throw std::logic_error("Section Did Not Retain Its Typed Value");

    google::protobuf::SimpleDescriptorDatabase database;
    for (const auto& file : catalog.types().file()) {
        if (!database.Add(file)) throw std::logic_error("Conflicting Catalog Descriptor");
    }
    google::protobuf::DescriptorPool pool(&database);
    for (const auto* name : {"harmos.v1.TimeSleepRequest", "harmos.v1.TimeSleepResponse", "harmos.v1.TimeGetRequest", "harmos.v1.TimeGetResponse"}) {
        if (!pool.FindMessageTypeByName(name)) throw std::logic_error("Required Message Does Not Resolve");
    }
    if (!pool.FindMessageTypeByName(catalog.sections(0).type())) throw std::logic_error("Section Type Does Not Resolve");
}

/* === Private Functions === */

/* === Entry Point === */

int main(int argc, char** argv) {
    try {
        if (argc != 2) throw std::invalid_argument("Expected a Test Name");
        const std::string name = argv[1];
        if (name == "catalog_rejects_duplicate_operations") catalog_rejects_duplicate_operations();
        else if (name == "observer_preserves_its_stream_contract") observer_preserves_its_stream_contract();
        else if (name == "catalog_preserves_typed_sections_and_dependencies") catalog_preserves_typed_sections_and_dependencies();
        else throw std::invalid_argument("Unknown Test");
        return 0;
    } catch (const std::exception& error) {
        std::cerr << error.what() << std::endl;
        return 1;
    }
}
