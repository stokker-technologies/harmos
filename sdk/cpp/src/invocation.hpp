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
#include <atomic>
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

namespace harmos::plugin::detail {
struct Invocation;
struct WatchState {
  std::weak_ptr<Invocation> owner;
  std::optional<::harmos::plugin::v1::ChildResponse> item;
  std::optional<::harmos::plugin::v1::ChildResponse> terminal;
  bool demanded = false;
  std::uint64_t max_item_bytes = 0;
};
struct Invocation {
  std::shared_ptr<Invocation> transport;
  std::shared_ptr<Invocation> caller;
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
  ::harmos::plugin::v1::Request request;
  std::string plugin;
  std::optional<std::string> cancelled;
  std::uint64_t next = 0;
  bool closed = false;
  bool tick_scoped = false;
  std::atomic<bool> tick_active = false;
  std::atomic<bool> accepting_children = true;

  bool receive(const ::harmos::plugin::v1::ChildResponse &child, bool allow_retired = true) {
    std::lock_guard lock(mutex);
    if (tick_scoped && !tick_active)
      return false;
    if (allow_retired && retired.contains(child.call()))
      return true;
    auto watching = watches.find(child.call());
    if (watching != watches.end()) {
      auto &watch = *watching->second;
      if (child.result_case() == ::harmos::plugin::v1::ChildResponse::RESULT_NOT_SET || child.item().size() > watch.max_item_bytes ||
          child.payload().size() > watch.max_item_bytes)
        return false;
      if (child.has_item()) {
        if (!watch.demanded || watch.item || watch.terminal)
          return false;
        watch.item = child;
      } else {
        if (watch.terminal)
          return false;
        watch.terminal = child;
      }
      changed.notify_all();
      return true;
    }
    auto found = pending.find(child.call());
    if (child.has_item() || found == pending.end() || child.result_case() == ::harmos::plugin::v1::ChildResponse::RESULT_NOT_SET)
      return false;
    if (child.has_failure())
      found->second.set_exception(std::make_exception_ptr(Failure(child.failure())));
    else
      found->second.set_value(child.payload());
    guards.erase(child.call());
    pending.erase(found);
    changed.notify_all();
    return true;
  }

  void start_worker(std::function<void()> work) {
    std::lock_guard lock(mutex);
    if (closed)
      throw TransportError(14, "Invoke: Invocation Closed");
    workers.emplace_back(std::move(work));
  }
  void settle_workers() {
    for (;;) {
      std::vector<std::thread> joining;
      {
        std::lock_guard lock(mutex);
        joining.swap(workers);
      }
      if (joining.empty())
        return;
      for (auto &worker : joining)
        if (worker.joinable())
          worker.join();
    }
  }
  void start_reading() {
    std::call_once(reading, [&] {
      if (read)
        read();
    });
  }
  void close(std::exception_ptr error) {
    std::lock_guard lock(mutex);
    closed = true;
    for (auto &[id, promise] : pending)
      promise.set_exception(error);
    pending.clear();
    if (!cancelled)
      cancelled = "Connection Closed";
    changed.notify_all();
  }
};
void cancel_children(const std::shared_ptr<Invocation> &state, const std::string &reason);
} // namespace harmos::plugin::detail

/* === Templates === */

/* === Public Declarations === */
