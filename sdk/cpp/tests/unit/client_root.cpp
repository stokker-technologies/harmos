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

namespace host_plugins_wire = harmos::host::v1::plugins;
namespace host_status_wire = harmos::host::v1::status;
namespace client_types = harmos::client;
namespace host_wire = harmos::host::v1;
namespace application_wire = application::host::v1;
using RpcStatus = grpc::Status;
using grpc::CallbackServerContext;
using grpc::CreateChannel;
using grpc::InsecureChannelCredentials;
using grpc::InsecureServerCredentials;
using grpc::ServerBuilder;
using grpc::ServerContext;
using grpc::ServerUnaryReactor;
using grpc::ServerWriter;
using grpc::StatusCode;
using std::string;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

namespace {
struct Status final : host_wire::Status::CallbackService {
  ServerUnaryReactor *Get(CallbackServerContext *context, const host_wire::StatusGetRequest *,
                          host_wire::StatusGetResponse *response) override {
    response->mutable_status()->mutable_identity()->set_application_name("status:selection");
    auto *reactor = context->DefaultReactor();
    reactor->Finish(RpcStatus::OK);
    return reactor;
  }
};
struct Diagnostics final : host_status_wire::Diagnostics::CallbackService {
  ServerUnaryReactor *Get(CallbackServerContext *context, const host_status_wire::DiagnosticsGetRequest *request,
                          host_status_wire::DiagnosticsGetResponse *response) override {
    response->set_name("diagnostics:" + request->name());
    auto *reactor = context->DefaultReactor();
    reactor->Finish(RpcStatus::OK);
    return reactor;
  }
};
struct Jobs final : application_wire::Jobs::Service {
  RpcStatus Start(ServerContext *, const application_wire::JobsStartRequest *request,
                  application_wire::JobsStartResponse *response) override {
    response->set_value("start:" + request->value());
    return RpcStatus::OK;
  }
};
struct Progress final : application_wire::jobs::Progress::Service {
  RpcStatus Watch(ServerContext *, const application_wire::jobs::ProgressWatchRequest *request,
                  ServerWriter<application_wire::jobs::ProgressWatchResponse> *output) override {
    for (const auto *phase : {"opened:", "finished:"}) {
      application_wire::jobs::ProgressWatchResponse response;
      response.set_value(string(phase) + request->value());
      if (!output->Write(response))
        return {StatusCode::CANCELLED, "Watch: Demand Closed"};
    }
    return RpcStatus::OK;
  }
};
struct Annotations final : application_wire::plugins::Annotations::Service {
  RpcStatus Get(ServerContext *, const application_wire::plugins::AnnotationsGetRequest *request,
                application_wire::plugins::AnnotationsGetResponse *response) override {
    response->set_value(request->name());
    return RpcStatus::OK;
  }
};
struct Settings final : host_plugins_wire::Settings::CallbackService {
  ServerUnaryReactor *List(CallbackServerContext *context, const host_plugins_wire::SettingsListRequest *request,
                           host_plugins_wire::SettingsListResponse *response) override {
    (*response->mutable_value()->mutable_values())["label"].set_text(request->name());
    response->set_revision("settings-revision");
    auto *reactor = context->DefaultReactor();
    reactor->Finish(RpcStatus::OK);
    return reactor;
  }
  ServerUnaryReactor *Set(CallbackServerContext *context, const host_plugins_wire::SettingsSetRequest *request,
                          host_plugins_wire::SettingsSetResponse *response) override {
    *response->mutable_value() = request->value();
    response->set_revision(request->request_id());
    auto *reactor = context->DefaultReactor();
    reactor->Finish(RpcStatus::OK);
    return reactor;
  }
};
struct Plugins final : host_wire::Plugins::CallbackService {
  ServerUnaryReactor *Replace(CallbackServerContext *context, const host_wire::PluginsReplaceRequest *request,
                              host_wire::PluginsReplaceResponse *response) override {
    response->mutable_entry()->set_name(request->name());
    *response->mutable_entry()->mutable_placement() = request->placement();
    auto *reactor = context->DefaultReactor();
    reactor->Finish(RpcStatus::OK);
    return reactor;
  }
};
struct Invocations final : host_wire::Invocations::CallbackService {
  ServerUnaryReactor *Create(CallbackServerContext *context, const host_wire::InvocationsCreateRequest *request,
                             host_wire::InvocationsCreateResponse *response) override {
    response->set_payload(request->operation() + request->fingerprint() + request->payload());
    auto *reactor = context->DefaultReactor();
    reactor->Finish(RpcStatus::OK);
    return reactor;
  }
};
} // namespace

/* === Tests === */

/* === Templates === */

/* === Public Functions === */

TEST_CASE("A client composes services and their nested resources") {
  SECTION("Library and Application Resources Share One Channel") {
    Status status;
    Diagnostics diagnostics;
    Jobs jobs;
    Progress progress;
    Annotations annotations;
    Settings settings;
    Plugins plugins;
    Invocations invocations;
    ServerBuilder builder;
    int port = 0;
    builder.AddListeningPort("127.0.0.1:0", InsecureServerCredentials(), &port);
    builder.RegisterService(&status);
    builder.RegisterService(&diagnostics);
    builder.RegisterService(&jobs);
    builder.RegisterService(&progress);
    builder.RegisterService(&annotations);
    builder.RegisterService(&settings);
    builder.RegisterService(&plugins);
    builder.RegisterService(&invocations);
    auto server = builder.BuildAndStart();
    REQUIRE(server);
    fixture::Client client(CreateChannel("127.0.0.1:" + std::to_string(port), InsecureChannelCredentials()));
    REQUIRE(client.status.get().status().identity().application_name() == "status:selection");
    REQUIRE(client.status.diagnostics.get("selection", 0, 512).name() == "diagnostics:selection");
    REQUIRE(client.jobs.start("selection") == "start:selection");
    auto watching = client.jobs.progress.watch("selection");
    REQUIRE(watching.next() == "opened:selection");
    REQUIRE(watching.next() == "finished:selection");
    REQUIRE_FALSE(watching.next());
    REQUIRE(client.plugins.annotations.get("example") == "example");
    REQUIRE(client.invocations.create("operation", string(32, '\0'), string("\0payload", 8)) ==
            "operation" + string(32, '\0') + string("\0payload", 8));
    const auto current = client.plugins.settings.list({.name = "example"});
    REQUIRE(current.value().values().at("label").text() == "example");
    auto values = current.value();
    (*values.mutable_values())["label"].set_text("replacement");
    REQUIRE(client.plugins.settings.set("example", values, current.revision(), {.request_id = "settings-replacement"})
                .value()
                .values()
                .at("label")
                .text() == "replacement");
    REQUIRE_FALSE(client.plugins.settings.set("example", values, current.revision()).revision().empty());
    REQUIRE(client.plugins.settings.set("example", values, current.revision(), {.request_id = "replay"}).revision() == "replay");
    client_types::PluginPlacement placement;
    placement.mutable_managed()->set_sha256(string(32, 'x'));
    const auto replaced = client.plugins.replace("example", placement, current.revision(), {.request_id = "bundle-replacement"});
    REQUIRE(replaced.entry().name() == "example");
    REQUIRE(replaced.entry().placement().managed().sha256() == string(32, 'x'));
    server->Shutdown();
    server->Wait();
  }
}
/* === Private Functions === */

/* === Entry Point === */
