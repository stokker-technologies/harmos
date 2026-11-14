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
#include <chrono>
#include <future>
#include <mutex>

// Third Party Imports
#include <catch2/catch_test_macros.hpp>

// Local Imports
#include "client.hpp"

namespace application_wire = application::host::v1;
namespace chrono = std::chrono;
using grpc::CallbackServerContext;
using grpc::ClientContext;
using grpc::CreateChannel;
using grpc::InsecureChannelCredentials;
using grpc::InsecureServerCredentials;
using grpc::ServerBuilder;
using grpc::ServerWriteReactor;
using grpc::StatusCode;
using std::lock_guard;
using std::mutex;
using std::promise;
using std::runtime_error;
using std::string;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

namespace {
class Watch final : public ServerWriteReactor<application_wire::jobs::ProgressWatchResponse> {
  mutex mutex_;
  bool writing_ = true;
  bool cancelled_ = false;
  promise<void> &cancelled;
  application_wire::jobs::ProgressWatchResponse opening;

public:
  Watch(CallbackServerContext *context, promise<void> &event) : cancelled(event) {
    const auto metadata = context->client_metadata().find("authorization");
    const auto token = metadata == context->client_metadata().end() ? "" : string(metadata->second.data(), metadata->second.size());
    const auto deadline = context->deadline() != chrono::system_clock::time_point::max();
    opening.set_value(token + (deadline ? ":deadline" : ":missing"));
    StartWrite(&opening);
  }
  void OnWriteDone(bool) override {
    lock_guard lock(mutex_);
    writing_ = false;
    if (cancelled_)
      Finish({StatusCode::CANCELLED, "Watch Progress: Caller Cancelled"});
  }
  void OnCancel() override {
    lock_guard lock(mutex_);
    cancelled_ = true;
    cancelled.set_value();
    if (!writing_)
      Finish({StatusCode::CANCELLED, "Watch Progress: Caller Cancelled"});
  }
  void OnDone() override {
    delete this;
  }
};
struct Progress final : application_wire::jobs::Progress::CallbackService {
  promise<void> cancelled;
  ServerWriteReactor<application_wire::jobs::ProgressWatchResponse> *Watch(CallbackServerContext *context,
                                                                           const application_wire::jobs::ProgressWatchRequest *) override {
    return new ::Watch(context, cancelled);
  }
};
} // namespace

/* === Templates === */

/* === Public Functions === */

TEST_CASE("One call preserves metadata deadline override and cancellation") {
  SECTION("Caller Controls Reach a Watch and Cancel It") {
    // Serve a Watch That Waits for the Caller to Cancel Its Context.
    Progress progress;
    auto cancelled = progress.cancelled.get_future();
    ServerBuilder builder;
    int port = 0;
    builder.AddListeningPort("127.0.0.1:0", InsecureServerCredentials(), &port);
    builder.RegisterService(&progress);
    auto server = builder.BuildAndStart();
    REQUIRE(server);
    fixture::Client client(CreateChannel("127.0.0.1:" + std::to_string(port), InsecureChannelCredentials()));
    // Own the Context for the Whole Stream and Supply All Three Per-Call Controls.
    auto context = std::make_shared<ClientContext>();
    context->AddMetadata("authorization", "Bearer fixture");
    context->set_deadline(chrono::system_clock::time_point(chrono::seconds(4102444800)));
    auto watching = client.jobs.progress.watch("selection", {.context = context});
    REQUIRE(watching.next() == "Bearer fixture:deadline");
    watching.cancel();
    REQUIRE_THROWS_AS(watching.next(), runtime_error);
    cancelled.get();
    server->Shutdown();
    server->Wait();
  }
}

/* === Private Functions === */

/* === Entry Point === */
