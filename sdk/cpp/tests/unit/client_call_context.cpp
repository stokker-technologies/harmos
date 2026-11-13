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


/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

namespace {
class Watch final : public grpc::ServerWriteReactor<application::v1::jobs::ProgressWatchResponse> {
    std::mutex mutex_;
    bool writing_ = true;
    bool cancelled_ = false;
    std::promise<void>& cancelled;
    application::v1::jobs::ProgressWatchResponse opening;
public:
    Watch(grpc::CallbackServerContext* context, std::promise<void>& event) : cancelled(event) {
        const auto metadata = context->client_metadata().find("authorization");
        const auto token = metadata == context->client_metadata().end() ? "" : std::string(metadata->second.data(), metadata->second.size());
        const auto deadline = context->deadline() != std::chrono::system_clock::time_point::max();
        opening.set_value(token + (deadline ? ":deadline" : ":missing"));
        StartWrite(&opening);
    }
    void OnWriteDone(bool) override {
        std::lock_guard lock(mutex_);
        writing_ = false;
        if (cancelled_) Finish({grpc::StatusCode::CANCELLED, "Watch Progress: Caller Cancelled"});
    }
    void OnCancel() override {
        std::lock_guard lock(mutex_);
        cancelled_ = true;
        cancelled.set_value();
        if (!writing_) Finish({grpc::StatusCode::CANCELLED, "Watch Progress: Caller Cancelled"});
    }
    void OnDone() override { delete this; }
};
struct Progress final : application::v1::jobs::Progress::CallbackService {
    std::promise<void> cancelled;
    grpc::ServerWriteReactor<application::v1::jobs::ProgressWatchResponse>* Watch(
        grpc::CallbackServerContext* context, const application::v1::jobs::ProgressWatchRequest*) override {
        return new ::Watch(context, cancelled);
    }
};
}


/* === Templates === */

/* === Public Functions === */

TEST_CASE("One call preserves metadata deadline override and cancellation") {
    SECTION("One call preserves metadata deadline override and cancellation") {
        // Serve a Watch That Waits for the Caller to Cancel Its Context.
        Progress progress;
        auto cancelled = progress.cancelled.get_future();
        grpc::ServerBuilder builder;
        int port = 0;
        builder.AddListeningPort("127.0.0.1:0", grpc::InsecureServerCredentials(), &port);
        builder.RegisterService(&progress);
        auto server = builder.BuildAndStart();
        REQUIRE(server);
        fixture::Client client(grpc::CreateChannel("127.0.0.1:" + std::to_string(port), grpc::InsecureChannelCredentials()));
        // Own the Context for the Whole Stream and Supply All Three Per-Call Controls.
        grpc::ClientContext context;
        context.AddMetadata("authorization", "Bearer fixture");
        context.set_deadline(std::chrono::system_clock::time_point(std::chrono::seconds(4102444800)));
        application::v1::jobs::ProgressWatchRequest request;
        auto watching = client.jobs.progress.watch(context, request);
        application::v1::jobs::ProgressWatchResponse opening;
        REQUIRE(watching->Read(&opening));
        REQUIRE(opening.value() == "Bearer fixture:deadline");
        context.TryCancel();
        const auto status = watching->Finish();
        REQUIRE(status.error_code() == grpc::StatusCode::CANCELLED);
        cancelled.get();
        server->Shutdown();
        server->Wait();
    }
}


/* === Private Functions === */

/* === Entry Point === */
