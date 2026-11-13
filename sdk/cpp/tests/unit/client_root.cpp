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

// Local Imports

#include "client.hpp"

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

namespace {
struct Status final : harmos::v1::Status::CallbackService {
    grpc::ServerUnaryReactor* Get(grpc::CallbackServerContext* context, const harmos::v1::StatusGetRequest*, harmos::v1::StatusGetResponse* response) override {
        response->mutable_status()->mutable_identity()->set_application_name("status:selection");
        auto* reactor = context->DefaultReactor();
        reactor->Finish(grpc::Status::OK);
        return reactor;
    }
};
struct Diagnostics final : harmos::v1::status::Diagnostics::CallbackService {
    grpc::ServerUnaryReactor* Get(grpc::CallbackServerContext* context, const harmos::v1::status::DiagnosticsGetRequest* request, harmos::v1::status::DiagnosticsGetResponse* response) override {
        response->set_name("diagnostics:" + request->name());
        auto* reactor = context->DefaultReactor();
        reactor->Finish(grpc::Status::OK);
        return reactor;
    }
};
struct Jobs final : application::v1::Jobs::Service {
    grpc::Status Start(grpc::ServerContext*, const application::v1::JobsStartRequest* request, application::v1::JobsStartResponse* response) override {
        response->set_value("start:" + request->value()); return grpc::Status::OK;
    }
};
struct Progress final : application::v1::jobs::Progress::Service {
    grpc::Status Watch(grpc::ServerContext*, const application::v1::jobs::ProgressWatchRequest* request, grpc::ServerWriter<application::v1::jobs::ProgressWatchResponse>* output) override {
        for (const auto* phase : {"opened:", "finished:"}) {
            application::v1::jobs::ProgressWatchResponse response; response.set_value(std::string(phase) + request->value());
            if (!output->Write(response)) return {grpc::StatusCode::CANCELLED, "Watch: Demand Closed"};
        }
        return grpc::Status::OK;
    }
};
}

/* === Tests === */

/* === Templates === */

/* === Public Functions === */

TEST_CASE("A client composes services and their nested resources") {
    SECTION("Library and Application Resources Share One Channel") {
        Status status; Diagnostics diagnostics; Jobs jobs; Progress progress;
        grpc::ServerBuilder builder;
        int port = 0;
        builder.AddListeningPort("127.0.0.1:0", grpc::InsecureServerCredentials(), &port);
        builder.RegisterService(&status); builder.RegisterService(&diagnostics); builder.RegisterService(&jobs); builder.RegisterService(&progress);
        auto server = builder.BuildAndStart();
        REQUIRE(server);
        fixture::Client client(grpc::CreateChannel("127.0.0.1:" + std::to_string(port), grpc::InsecureChannelCredentials()));
        harmos::v1::StatusGetRequest status_request;
        grpc::ClientContext status_context;
        REQUIRE(client.status.get(status_context, status_request).status().identity().application_name() == "status:selection");
        harmos::v1::status::DiagnosticsGetRequest diagnostics_request;
        diagnostics_request.set_name("selection");
        grpc::ClientContext diagnostics_context;
        REQUIRE(client.status.diagnostics.get(diagnostics_context, diagnostics_request).name() == "diagnostics:selection");
        application::v1::JobsStartRequest job_request; job_request.set_value("selection");
        grpc::ClientContext job_context;
        REQUIRE(client.jobs.start(job_context, job_request).value() == "start:selection");
        application::v1::jobs::ProgressWatchRequest progress_request; progress_request.set_value("selection");
        grpc::ClientContext progress_context;
        auto watching = client.jobs.progress.watch(progress_context, progress_request);
        application::v1::jobs::ProgressWatchResponse item;
        REQUIRE(watching->Read(&item));
        REQUIRE(item.value() == "opened:selection");
        REQUIRE(watching->Read(&item));
        REQUIRE(item.value() == "finished:selection");
        REQUIRE_FALSE(watching->Read(&item));
        REQUIRE(watching->Finish().ok());
        server->Shutdown(); server->Wait();
    }
}
/* === Private Functions === */

/* === Entry Point === */
