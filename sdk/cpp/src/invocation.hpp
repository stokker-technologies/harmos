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
#include <set>
#include <thread>

// Third Party Imports

// Local Imports
#include "harmos/harmos.hpp"
#include "harmos/plugin/v1/instance.pb.h"

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

namespace harmos::detail {
struct Invocation;
struct WatchState {
    std::weak_ptr<Invocation> owner;
    std::optional<wire::ChildResponse> item;
    std::optional<wire::ChildResponse> terminal;
    bool demanded = false;
    std::uint64_t max_item_bytes = 0;
};
struct Invocation {
    std::shared_ptr<Invocation> transport;
    std::weak_ptr<Invocation> caller;
    std::set<std::uint64_t> owned;
    std::set<std::uint64_t> retired;
    std::set<std::uint64_t> guards;
    std::vector<std::thread> workers;
    bool background = false;
    std::mutex mutex;
    std::condition_variable changed;
    std::map<std::uint64_t, std::promise<std::string>> pending;
    std::map<std::uint64_t, std::shared_ptr<WatchState>> watches;
    std::map<std::string, std::string> requirements;
    std::function<void(plugin::v1::InstanceInvokeResponse)> emit;
    std::function<void()> read;
    std::once_flag reading;
    std::map<std::string, OperationDefinition> local;
    wire::Request request;
    std::string plugin;
    std::optional<std::string> cancelled;
    std::uint64_t next = 0;
    bool closed = false;

    void start_worker(std::function<void()> work) {
        std::lock_guard lock(mutex);
        if (closed) throw TransportError(14, "Invoke: Invocation Closed");
        workers.emplace_back(std::move(work));
    }
    void settle_workers() {
        for (;;) {
            std::vector<std::thread> joining;
            { std::lock_guard lock(mutex); joining.swap(workers); }
            if (joining.empty()) return;
            for (auto& worker : joining) if (worker.joinable()) worker.join();
        }
    }
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
