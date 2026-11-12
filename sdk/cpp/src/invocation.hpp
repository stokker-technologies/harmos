/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#pragma once

/* === Imports === */

// Standard Library Imports
#include <condition_variable>
#include <map>
#include <mutex>

// Third Party Imports

// Local Imports
#include "harmos/harmos.hpp"

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

namespace harmos::detail {
struct Invocation {
    std::mutex mutex;
    std::condition_variable changed;
    std::map<std::uint64_t, std::promise<std::string>> pending;
    std::map<std::string, std::string> requirements;
    std::function<void(wire::PluginInvokeResponse)> emit;
    std::function<void()> read;
    std::once_flag reading;
    wire::Request request;
    std::string plugin;
    std::optional<std::string> cancelled;
    std::uint64_t next = 0;
    bool closed = false;

    void start_reading() { std::call_once(reading, [&] { if (read) read(); }); }
    void close(std::exception_ptr error) {
        std::lock_guard lock(mutex);
        closed = true;
        for (auto& [id, promise] : pending) promise.set_exception(error);
        pending.clear();
        if (!cancelled) cancelled = "Connection Closed";
        changed.notify_all();
    }
};
}

/* === Templates === */

/* === Public Declarations === */
