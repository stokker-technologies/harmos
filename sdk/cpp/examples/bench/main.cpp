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
#include <array>
#include <iostream>
#include <thread>

// Third Party Imports

// Local Imports
#include <harmos/harmos.hpp>


/* === Macros === */

/* === Constants === */

/* === Types === */

namespace bench {
using namespace std::chrono_literals;

struct ReadTemperature {
    std::int32_t channel;
    struct Response { double temperature_c; };
    static constexpr auto id = "bench.read_temperature";
    static constexpr harmos::Policy policy{.timeout = 5s, .attempt_timeout = 5s};
    Response run(harmos::Context& ctx) const {
        ctx.check_cancelled();
        if (channel < 0) throw std::runtime_error("channel is unavailable");
        return {20.0 + 0.5 * channel};
    }
};

struct Capture {
    std::int32_t channel;
    std::uint32_t sample_count;
    struct Response { std::uint32_t published; };
    static constexpr auto id = "bench.capture";
    static constexpr harmos::Policy policy{.timeout = 5s, .attempt_timeout = 5s};
    static constexpr std::array streams{harmos::Stream{"temperature", 8}};
    Response run(harmos::Context& ctx) const;
};
} // namespace bench

HARMOS_FIELDS(bench::ReadTemperature, channel)
HARMOS_FIELDS(bench::ReadTemperature::Response, temperature_c)
HARMOS_FIELDS(bench::Capture, channel, sample_count)
HARMOS_FIELDS(bench::Capture::Response, published)


/* === Classes === */

/* === Templates === */

/* === Public Functions === */

bench::Capture::Response bench::Capture::run(harmos::Context& ctx) const {
    // Read The Simulated Driver Through The Host.
    const auto reading = ctx.operation(ReadTemperature{channel});
    ctx.check_cancelled();

    // Build A Batch With nanoarrow.
    harmos::Batch batch;
    auto check = [](int status) { if (status != NANOARROW_OK) throw std::runtime_error("batch construction failed"); };
    ArrowSchemaInit(batch.schema.get());
    check(ArrowSchemaSetTypeStruct(batch.schema.get(), 2));
    check(ArrowSchemaSetType(batch.schema->children[0], NANOARROW_TYPE_INT32));
    check(ArrowSchemaSetName(batch.schema->children[0], "channel"));
    check(ArrowSchemaSetType(batch.schema->children[1], NANOARROW_TYPE_DOUBLE));
    check(ArrowSchemaSetName(batch.schema->children[1], "temperature_c"));
    check(ArrowArrayInitFromSchema(batch.array.get(), batch.schema.get(), nullptr));
    check(ArrowArrayStartAppending(batch.array.get()));
    const auto count = sample_count == 0 ? 1 : sample_count;
    for (std::uint32_t index = 0; index < count; ++index) {
        ctx.check_cancelled();
        check(ArrowArrayAppendInt(batch.array->children[0], channel));
        check(ArrowArrayAppendDouble(batch.array->children[1], reading.temperature_c + 0.25 * index));
        check(ArrowArrayFinishElement(batch.array.get()));
    }
    check(ArrowArrayFinishBuildingDefault(batch.array.get(), nullptr));
    ctx.publish("temperature", batch);

    // A Zero Count Keeps Acquisition Open Until Cancellation.
    while (sample_count == 0) {
        ctx.check_cancelled();
        std::this_thread::yield();
    }
    return {count};
}

int main() {
    try {
        // Declare The Plugin's Operations.
        harmos::Plugin plugin{"bench"};
        plugin.operation<bench::ReadTemperature>();
        plugin.operation<bench::Capture>();

        // Bootstrap, Authenticate And Serve Until The Host Retires This Generation.
        return plugin.serve();
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}


/* === Private Functions === */

/* === Entry Point === */
