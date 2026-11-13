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
struct Plugins final : library::v1::Plugins::Service {
    grpc::Status Get(grpc::ServerContext*, const library::v1::PluginsGetRequest* request, library::v1::PluginsGetResponse* response) override {
        response->set_value("plugins:" + request->value()); return grpc::Status::OK;
    }
};
struct Settings final : library::v1::plugins::Settings::Service {
    grpc::Status Get(grpc::ServerContext*, const library::v1::plugins::SettingsGetRequest* request, library::v1::plugins::SettingsGetResponse* response) override {
        response->set_value("settings:" + request->value()); return grpc::Status::OK;
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
    SECTION("A client composes services and their nested resources") {
        Plugins plugins; Settings settings; Jobs jobs; Progress progress;
        grpc::ServerBuilder builder;
        int port = 0;
        builder.AddListeningPort("127.0.0.1:0", grpc::InsecureServerCredentials(), &port);
        builder.RegisterService(&plugins); builder.RegisterService(&settings); builder.RegisterService(&jobs); builder.RegisterService(&progress);
        auto server = builder.BuildAndStart();
        REQUIRE(server);
        fixture::Client client(grpc::CreateChannel("127.0.0.1:" + std::to_string(port), grpc::InsecureChannelCredentials()));
        library::v1::PluginsGetRequest plugin_request; plugin_request.set_value("selection");
        grpc::ClientContext plugin_context;
        REQUIRE(client.plugins.get(plugin_context, plugin_request).value() == "plugins:selection");
        library::v1::plugins::SettingsGetRequest setting_request; setting_request.set_value("selection");
        grpc::ClientContext setting_context;
        REQUIRE(client.plugins.settings.get(setting_context, setting_request).value() == "settings:selection");
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
