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
struct Status final : harmos::client::v1::Status::CallbackService {
    grpc::ServerUnaryReactor* Get(grpc::CallbackServerContext* context, const harmos::client::v1::StatusGetRequest*, harmos::client::v1::StatusGetResponse* response) override {
        response->mutable_status()->mutable_identity()->set_application_name("status:selection");
        auto* reactor = context->DefaultReactor();
        reactor->Finish(grpc::Status::OK);
        return reactor;
    }
};
struct Diagnostics final : harmos::client::v1::status::Diagnostics::CallbackService {
    grpc::ServerUnaryReactor* Get(grpc::CallbackServerContext* context, const harmos::client::v1::status::DiagnosticsGetRequest* request, harmos::client::v1::status::DiagnosticsGetResponse* response) override {
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
struct Annotations final : application::v1::plugins::Annotations::Service {
    grpc::Status Get(grpc::ServerContext*, const application::v1::plugins::AnnotationsGetRequest* request,
                     application::v1::plugins::AnnotationsGetResponse* response) override {
        response->set_value(request->name());
        return grpc::Status::OK;
    }
};
struct Settings final : harmos::client::v1::plugins::Settings::CallbackService {
    grpc::ServerUnaryReactor* List(grpc::CallbackServerContext* context, const harmos::client::v1::plugins::SettingsListRequest* request,
                                  harmos::client::v1::plugins::SettingsListResponse* response) override {
        (*response->mutable_value()->mutable_values())["label"].set_text(request->name());
        response->set_revision("settings-revision");
        auto* reactor = context->DefaultReactor();
        reactor->Finish(grpc::Status::OK);
        return reactor;
    }
    grpc::ServerUnaryReactor* Set(grpc::CallbackServerContext* context, const harmos::client::v1::plugins::SettingsSetRequest* request,
                                 harmos::client::v1::plugins::SettingsSetResponse* response) override {
        *response->mutable_value() = request->value();
        response->set_revision(request->expected_revision());
        auto* reactor = context->DefaultReactor();
        reactor->Finish(grpc::Status::OK);
        return reactor;
    }
};
struct Plugins final : harmos::client::v1::Plugins::CallbackService {
    grpc::ServerUnaryReactor* Replace(grpc::CallbackServerContext* context, const harmos::client::v1::PluginsReplaceRequest* request,
                                     harmos::client::v1::PluginsReplaceResponse* response) override {
        response->mutable_entry()->set_name(request->name());
        *response->mutable_entry()->mutable_placement() = request->placement();
        auto* reactor = context->DefaultReactor();
        reactor->Finish(grpc::Status::OK);
        return reactor;
    }
};
}

/* === Tests === */

/* === Templates === */

/* === Public Functions === */

TEST_CASE("A client composes services and their nested resources") {
    SECTION("Library and Application Resources Share One Channel") {
        Status status; Diagnostics diagnostics; Jobs jobs; Progress progress; Annotations annotations; Settings settings; Plugins plugins;
        grpc::ServerBuilder builder;
        int port = 0;
        builder.AddListeningPort("127.0.0.1:0", grpc::InsecureServerCredentials(), &port);
        builder.RegisterService(&status); builder.RegisterService(&diagnostics); builder.RegisterService(&jobs); builder.RegisterService(&progress);
        builder.RegisterService(&annotations); builder.RegisterService(&settings); builder.RegisterService(&plugins);
        auto server = builder.BuildAndStart();
        REQUIRE(server);
        fixture::Client client(grpc::CreateChannel("127.0.0.1:" + std::to_string(port), grpc::InsecureChannelCredentials()));
        harmos::client::v1::StatusGetRequest status_request;
        grpc::ClientContext status_context;
        REQUIRE(client.status.get(status_context, status_request).status().identity().application_name() == "status:selection");
        harmos::client::v1::status::DiagnosticsGetRequest diagnostics_request;
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
        application::v1::plugins::AnnotationsGetRequest annotation_request;
        annotation_request.set_name("example");
        grpc::ClientContext annotation_context;
        REQUIRE(client.plugins.annotations.get(annotation_context, annotation_request).value() == "example");
        harmos::client::v1::plugins::SettingsListRequest settings_request;
        settings_request.set_name("example");
        grpc::ClientContext settings_context;
        const auto current = client.plugins.settings.list(settings_context, settings_request);
        REQUIRE(current.value().values().at("label").text() == "example");
        harmos::client::v1::plugins::SettingsSetRequest replacement;
        replacement.set_name("example");
        replacement.set_request_id("settings-replacement");
        replacement.set_expected_revision(current.revision());
        *replacement.mutable_value() = current.value();
        (*replacement.mutable_value()->mutable_values())["label"].set_text("replacement");
        grpc::ClientContext replacement_context;
        REQUIRE(client.plugins.settings.set(replacement_context, replacement).value().values().at("label").text() == "replacement");
        harmos::client::v1::PluginsReplaceRequest bundle;
        bundle.set_name("example");
        bundle.set_request_id("bundle-replacement");
        bundle.mutable_placement()->mutable_managed()->set_sha256(std::string(32, 'x'));
        grpc::ClientContext bundle_context;
        const auto replaced = client.plugins.replace(bundle_context, bundle);
        REQUIRE(replaced.entry().name() == "example");
        REQUIRE(replaced.entry().placement().managed().sha256() == std::string(32, 'x'));
        server->Shutdown(); server->Wait();
    }
}
/* === Private Functions === */

/* === Entry Point === */
