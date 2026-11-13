// @generated from the protocol descriptor table; do not edit.
#pragma once
#include <grpcpp/grpcpp.h>
#include <memory>
#include <stdexcept>
#pragma push_macro("Get")
#undef Get
#pragma push_macro("Start")
#undef Start
#pragma push_macro("Watch")
#undef Watch
#include "application/v1/jobs.grpc.pb.h"
#include "application/v1/jobs/progress.grpc.pb.h"
#include "library/v1/plugins.grpc.pb.h"
#include "library/v1/plugins/settings.grpc.pb.h"
namespace fixture {
struct JobsProgressResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit JobsProgressResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  std::unique_ptr<grpc::ClientReader<::application::v1::jobs::ProgressWatchResponse>> watch(grpc::ClientContext& context, const ::application::v1::jobs::ProgressWatchRequest& request) {
    return ::application::v1::jobs::Progress::NewStub(channel)->Watch(&context, request);
  }
};
struct PluginsSettingsResource {
  std::shared_ptr<grpc::Channel> channel;
  explicit PluginsSettingsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel) {}
  ::library::v1::plugins::SettingsGetResponse get(grpc::ClientContext& context, const ::library::v1::plugins::SettingsGetRequest& request) {
    ::library::v1::plugins::SettingsGetResponse response;
    const auto status = ::library::v1::plugins::Settings::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
};
struct JobsResource {
  std::shared_ptr<grpc::Channel> channel;
  JobsProgressResource progress;
  explicit JobsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel), progress(channel) {}
  ::application::v1::JobsStartResponse start(grpc::ClientContext& context, const ::application::v1::JobsStartRequest& request) {
    ::application::v1::JobsStartResponse response;
    const auto status = ::application::v1::Jobs::NewStub(channel)->Start(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Start: ") + status.error_message());
    return response;
  }
};
struct PluginsResource {
  std::shared_ptr<grpc::Channel> channel;
  PluginsSettingsResource settings;
  explicit PluginsResource(std::shared_ptr<grpc::Channel> channel) : channel(channel), settings(channel) {}
  ::library::v1::PluginsGetResponse get(grpc::ClientContext& context, const ::library::v1::PluginsGetRequest& request) {
    ::library::v1::PluginsGetResponse response;
    const auto status = ::library::v1::Plugins::NewStub(channel)->Get(&context, request, &response);
    if (!status.ok()) throw std::runtime_error(std::string("Get: ") + status.error_message());
    return response;
  }
};
struct Client {
  JobsResource jobs;
  PluginsResource plugins;
  explicit Client(std::shared_ptr<grpc::Channel> channel) : jobs(channel), plugins(channel) {}
};
}
#pragma pop_macro("Watch")
#pragma pop_macro("Start")
#pragma pop_macro("Get")
