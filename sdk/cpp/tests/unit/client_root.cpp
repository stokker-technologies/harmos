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

using grpc::CallbackServerContext;
using grpc::CreateChannel;
using grpc::InsecureChannelCredentials;
using grpc::InsecureServerCredentials;
using grpc::ServerBuilder;
using grpc::ServerContext;
using grpc::ServerUnaryReactor;
using grpc::ServerWriteReactor;
using grpc::StatusCode;
using std::string;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

namespace {
struct Status final : application::host::v1::Status::CallbackService {
  ServerUnaryReactor *Get(CallbackServerContext *context, const harmos::host::v1::StatusGetRequest *,
                          harmos::host::v1::StatusGetResponse *response) override {
    response->mutable_status()->mutable_identity()->set_application_name("status:selection");
    auto *reactor = context->DefaultReactor();
    reactor->Finish(grpc::Status::OK);
    return reactor;
  }
};
struct Diagnostics final : application::host::v1::status::Diagnostics::CallbackService {
  ServerUnaryReactor *Get(CallbackServerContext *context, const harmos::host::v1::status::DiagnosticsGetRequest *request,
                          harmos::host::v1::status::DiagnosticsGetResponse *response) override {
    response->set_name("diagnostics:" + request->name());
    auto *reactor = context->DefaultReactor();
    reactor->Finish(grpc::Status::OK);
    return reactor;
  }
};
struct Jobs final : application::host::v1::Jobs::Service {
  grpc::Status Start(ServerContext *, const application::host::v1::JobsStartRequest *request,
                     application::host::v1::JobsStartResponse *response) override {
    response->set_value("start:" + request->value());
    return grpc::Status::OK;
  }
};
class ProgressWatch final : public ServerWriteReactor<application::host::v1::jobs::ProgressWatchResponse> {
  application::host::v1::jobs::ProgressWatchResponse response_;
  string value_;
  bool finished_ = false;

public:
  explicit ProgressWatch(string value) : value_(std::move(value)) {
    response_.set_value("opened:" + value_);
    StartWrite(&response_);
  }
  void OnWriteDone(bool ok) override {
    if (!ok) {
      Finish({StatusCode::CANCELLED, "Watch: Demand Closed"});
      return;
    }
    if (finished_) {
      Finish(grpc::Status::OK);
      return;
    }
    response_.set_value("finished:" + value_);
    finished_ = true;
    StartWrite(&response_);
  }
  void OnDone() override {
    delete this;
  }
};
struct Progress final : application::host::v1::jobs::Progress::CallbackService {
  ServerWriteReactor<application::host::v1::jobs::ProgressWatchResponse> *
  Watch(CallbackServerContext *, const application::host::v1::jobs::ProgressWatchRequest *request) override {
    return new ProgressWatch(request->value());
  }
};
struct Annotations final : application::host::v1::plugins::Annotations::Service {
  grpc::Status Get(ServerContext *, const application::host::v1::plugins::AnnotationsGetRequest *request,
                   application::host::v1::plugins::AnnotationsGetResponse *response) override {
    response->set_value(request->name());
    return grpc::Status::OK;
  }
};
struct Settings final : application::host::v1::plugins::Settings::CallbackService {
  ServerUnaryReactor *List(CallbackServerContext *context, const harmos::host::v1::plugins::SettingsListRequest *request,
                           harmos::host::v1::plugins::SettingsListResponse *response) override {
    (*response->mutable_value()->mutable_values())["label"].set_text(request->name());
    response->set_revision("settings-revision");
    auto *reactor = context->DefaultReactor();
    reactor->Finish(grpc::Status::OK);
    return reactor;
  }
  ServerUnaryReactor *Set(CallbackServerContext *context, const harmos::host::v1::plugins::SettingsSetRequest *request,
                          harmos::host::v1::plugins::SettingsSetResponse *response) override {
    *response->mutable_value() = request->value();
    response->set_revision(request->request_id());
    auto *reactor = context->DefaultReactor();
    reactor->Finish(grpc::Status::OK);
    return reactor;
  }
};
struct Plugins final : application::host::v1::Plugins::CallbackService {
  ServerUnaryReactor *Replace(CallbackServerContext *context, const harmos::host::v1::PluginsReplaceRequest *request,
                              harmos::host::v1::PluginsReplaceResponse *response) override {
    response->mutable_entry()->set_name(request->name());
    *response->mutable_entry()->mutable_placement() = request->placement();
    auto *reactor = context->DefaultReactor();
    reactor->Finish(grpc::Status::OK);
    return reactor;
  }
};
struct Invocations final : application::host::v1::Invocations::CallbackService {
  ServerUnaryReactor *Create(CallbackServerContext *context, const harmos::host::v1::InvocationsCreateRequest *request,
                             harmos::host::v1::InvocationsCreateResponse *response) override {
    response->set_payload(request->operation() + request->fingerprint() + request->payload());
    auto *reactor = context->DefaultReactor();
    reactor->Finish(grpc::Status::OK);
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
    const auto current = client.plugins.settings.list({.name = "example", .at = {}, .context = {}});
    REQUIRE(current.value().values().at("label").text() == "example");
    auto values = current.value();
    (*values.mutable_values())["label"].set_text("replacement");
    REQUIRE(client.plugins.settings.set("example", values, current.revision(), {.request_id = "settings-replacement", .context = {}})
                .value()
                .values()
                .at("label")
                .text() == "replacement");
    REQUIRE_FALSE(client.plugins.settings.set("example", values, current.revision()).revision().empty());
    REQUIRE(client.plugins.settings.set("example", values, current.revision(), {.request_id = "replay", .context = {}}).revision() ==
            "replay");
    harmos::client::PluginPlacement placement;
    placement.mutable_managed()->set_sha256(string(32, 'x'));
    const auto replaced =
        client.plugins.replace("example", placement, current.revision(), {.request_id = "bundle-replacement", .context = {}});
    REQUIRE(replaced.entry().name() == "example");
    REQUIRE(replaced.entry().placement().managed().sha256() == string(32, 'x'));
    server->Shutdown();
    server->Wait();
  }
}
/* === Private Functions === */

/* === Entry Point === */
