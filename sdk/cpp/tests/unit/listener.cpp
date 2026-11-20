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
#include <atomic>
#include <future>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <utility>

// Third Party Imports
#include <catch2/catch_test_macros.hpp>
#include <grpc/impl/channel_arg_names.h>
#include <grpcpp/grpcpp.h>

// Local Imports
#include "generated/types.hpp"
#include "harmos/harmos.hpp"
#include "harmos/plugin/v1/instance.grpc.pb.h"

using grpc::ChannelArguments;
using grpc::ClientContext;
using grpc::CreateChannel;
using grpc::CreateCustomChannel;
using grpc::InsecureChannelCredentials;
using grpc::StatusCode;
using harmos::conformance::Value;
using harmos::plugin::Context;
using harmos::plugin::Listener;
using harmos::plugin::Operation;
using harmos::plugin::Plugin;
using harmos::plugin::Registrar;
using harmos::plugin::v1::Instance;
using harmos::plugin::v1::InstanceGetRequest;
using harmos::plugin::v1::InstanceGetResponse;
using harmos::plugin::v1::InstanceInvokeRequest;
using harmos::plugin::v1::InstanceInvokeResponse;
using harmos::plugin::v1::InstanceStopRequest;
using harmos::plugin::v1::InstanceStopResponse;
using std::async;
using std::atomic;
using std::launch;
using std::ostringstream;
using std::promise;
using std::runtime_error;
using std::stoi;
using std::string;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

TEST_CASE("Starting an Addressable Listener Writes No Output") {
  ostringstream output;
  auto *original = std::cout.rdbuf(output.rdbuf());
  try {
    auto listener = Plugin("example", "1.0.0").start("127.0.0.1:0");
    listener.close();
  } catch (...) {
    std::cout.rdbuf(original);
    throw;
  }
  std::cout.rdbuf(original);
  REQUIRE(output.str().empty());
}

TEST_CASE("An Owned Listener Drains Invocations and Restarts") {
  // Gate the Handler With Events, Including a Once-Only Cleanup Observation.
  promise<void> entered;
  promise<void> release;
  promise<void> cleanup_entered;
  const auto released = release.get_future().share();
  atomic<bool> settled = false;
  atomic<bool> close_returned = false;
  atomic<int> stopped = 0;
  Operation<Value, Value> echo("example.echo", harmos::VISIBILITY_PUBLIC, [&](Context &, Value request) {
    entered.set_value();
    released.wait();
    settled = true;
    return request;
  });
  Plugin plugin("example", "1.0.0");
  plugin
      .add([&](Registrar &catalog) {
        catalog.add(echo);
      })
      .on_stop([&] {
        ++stopped;
        cleanup_entered.set_value();
      });

  // Bind Port Zero and Invoke Through the Reported Concrete Address.
  auto listener = plugin.start("127.0.0.1:0");
  const auto address = listener.address();
  REQUIRE(address.starts_with("127.0.0.1:"));
  REQUIRE(stoi(address.substr(address.rfind(':') + 1)) > 0);
  auto stub = Instance::NewStub(CreateChannel(address, InsecureChannelCredentials()));
  ClientContext context;
  auto call = stub->Invoke(&context);
  InstanceInvokeRequest opening;
  opening.mutable_request()->set_operation("example.echo");
  opening.mutable_request()->set_invocation("invocation");
  opening.mutable_request()->set_attempt(1);
  Value request;
  request.set_value("hello");
  opening.mutable_request()->set_payload(request.SerializeAsString());
  REQUIRE(call->Write(opening));
  REQUIRE(call->WritesDone());
  entered.get_future().get();

  // Request Close While the Invocation Is Still Accepted, Then Let It Settle.
  auto closed = async(launch::async, [&] {
    listener.close();
    close_returned = true;
  });
  cleanup_entered.get_future().get();
  REQUIRE_FALSE(settled);
  REQUIRE_FALSE(close_returned);
  release.set_value();
  InstanceInvokeResponse frame;
  REQUIRE(call->Read(&frame));
  REQUIRE(frame.has_response());
  REQUIRE(frame.response().has_payload());
  Value response;
  REQUIRE(response.ParseFromString(frame.response().payload()));
  REQUIRE(response.value() == "hello");
  REQUIRE_FALSE(call->Read(&frame));
  REQUIRE(call->Finish().ok());
  closed.get();
  REQUIRE(settled);
  REQUIRE(close_returned);
  REQUIRE(stopped == 1);

  // Repeated Close Is a No-Op and a Closed Listener Refuses Connections.
  listener.close();
  REQUIRE(stopped == 1);
  ClientContext disconnected;
  InstanceGetRequest get;
  get.set_revision(1);
  InstanceGetResponse description;
  REQUIRE(stub->Get(&disconnected, get, &description).error_code() == StatusCode::UNAVAILABLE);

  // The Same Process Can Bind the Released Address Again.
  auto second = Plugin("second", "1.0.0").start(address);
  REQUIRE(second.address() == address);
  // Isolate the New Transport From the Closed Channel's Shared Connection Backoff.
  ChannelArguments arguments;
  arguments.SetInt(GRPC_ARG_USE_LOCAL_SUBCHANNEL_POOL, 1);
  auto second_stub = Instance::NewStub(CreateCustomChannel(second.address(), InsecureChannelCredentials(), arguments));
  ClientContext connected;
  const auto status = second_stub->Get(&connected, get, &description);
  INFO("Restart Get: " << status.error_code() << ": " << status.error_message());
  REQUIRE(status.ok());
  REQUIRE(description.id() == "second");
  second.close();
}

TEST_CASE("Remote Stop and Listener Close Run Cleanup Once") {
  atomic<int> stopped = 0;
  Plugin plugin("remote", "1.0.0");
  plugin.on_stop([&] {
    ++stopped;
  });
  auto listener = plugin.start("127.0.0.1:0");
  auto stub = Instance::NewStub(CreateChannel(listener.address(), InsecureChannelCredentials()));
  InstanceStopRequest request;
  InstanceStopResponse response;
  ClientContext first;
  REQUIRE(stub->Stop(&first, request, &response).ok());
  ClientContext repeated;
  REQUIRE(stub->Stop(&repeated, request, &response).ok());
  InstanceGetRequest get;
  get.set_revision(1);
  InstanceGetResponse description;
  ClientContext connected;
  REQUIRE(stub->Get(&connected, get, &description).ok());
  listener.close();
  listener.close();
  REQUIRE(stopped == 1);
}

TEST_CASE("Listener Destruction and Moves Own Shutdown") {
  atomic<int> stopped = 0;
  Plugin plugin("example", "1.0.0");
  plugin.on_stop([&] {
    ++stopped;
  });
  string address;
  {
    auto listener = plugin.start("127.0.0.1:0");
    address = listener.address();
    Listener moved(std::move(listener));
    listener.close();
    auto replacement = plugin.start("127.0.0.1:0");
    replacement = std::move(moved);
    REQUIRE(stopped == 1);
  }
  REQUIRE(stopped == 2);
  auto listener = plugin.start(address);
  listener.close();
  REQUIRE(stopped == 3);
}

TEST_CASE("Cleanup Failure Still Releases the Listener") {
  atomic<int> stopped = 0;
  Plugin plugin("failure", "1.0.0");
  plugin.on_stop([&] {
    ++stopped;
    throw runtime_error("Stop Plugin: Release the Device");
  });
  auto listener = plugin.start("127.0.0.1:0");
  const auto address = listener.address();
  REQUIRE_THROWS_AS(listener.close(), runtime_error);
  REQUIRE_NOTHROW(listener.close());
  REQUIRE(stopped == 1);
  auto second = Plugin("second", "1.0.0").start(address);
  second.close();
}

/* === Private Functions === */

/* === Entry Point === */
