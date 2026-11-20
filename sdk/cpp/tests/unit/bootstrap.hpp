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
#include <cstdlib>
#include <memory>
#include <mutex>
#include <string>
#include <utility>
#include <vector>

// Third Party Imports
#include <grpcpp/grpcpp.h>

// Local Imports
#include "harmos/plugin/v1/launches.grpc.pb.h"

/* === Macros === */

/* === Constants === */

/* === Types === */

namespace harmos::tests {

// Set a Variable for This Process; a Null Value Removes It.
inline void set_environment(const char *name, const char *value) {
#ifdef _WIN32
  _putenv_s(name, value ? value : "");
#else
  if (value)
    setenv(name, value, 1);
  else
    unsetenv(name);
#endif
}

/* === Classes === */

// A Minimal Host Bootstrap Listener: Serves Launches/Announce on Loopback, Records Each Call and
// Exports Both Launch Variables While It Lives. A Refusal Answers Every Call With UNAUTHENTICATED.
class Bootstrap final : public harmos::plugin::v1::Launches::CallbackService {
public:
  static constexpr const char *token = "story-launch-token";

  explicit Bootstrap(std::string refusal = {}) : refusal_(std::move(refusal)) {
    grpc::ServerBuilder builder;
    builder.AddListeningPort("127.0.0.1:0", grpc::InsecureServerCredentials(), &port_);
    builder.RegisterService(this);
    server_ = builder.BuildAndStart();
    set_environment("HARMOS_HOST_ENDPOINT", ("127.0.0.1:" + std::to_string(port_)).c_str());
    set_environment("HARMOS_LAUNCH_TOKEN", token);
  }

  Bootstrap(const Bootstrap &) = delete;
  Bootstrap &operator=(const Bootstrap &) = delete;

  ~Bootstrap() override {
    set_environment("HARMOS_HOST_ENDPOINT", nullptr);
    set_environment("HARMOS_LAUNCH_TOKEN", nullptr);
    server_->Shutdown();
  }

  harmos::plugin::v1::LaunchesAnnounceRequest announced() {
    std::unique_lock lock(mutex_);
    changed_.wait(lock, [this] {
      return !announces_.empty();
    });
    return announces_.front();
  }

  std::size_t count() {
    std::lock_guard lock(mutex_);
    return announces_.size();
  }

  grpc::ServerUnaryReactor *Announce(grpc::CallbackServerContext *context, const harmos::plugin::v1::LaunchesAnnounceRequest *request,
                                     harmos::plugin::v1::LaunchesAnnounceResponse *) override {
    {
      std::lock_guard lock(mutex_);
      announces_.push_back(*request);
    }
    changed_.notify_all();
    auto *reactor = context->DefaultReactor();
    reactor->Finish(refusal_.empty() ? grpc::Status::OK : grpc::Status(grpc::StatusCode::UNAUTHENTICATED, refusal_));
    return reactor;
  }

private:
  std::string refusal_;
  int port_ = 0;
  std::unique_ptr<grpc::Server> server_;
  std::mutex mutex_;
  std::condition_variable changed_;
  std::vector<harmos::plugin::v1::LaunchesAnnounceRequest> announces_;
};
} // namespace harmos::tests

/* === Templates === */

/* === Public Declarations === */
