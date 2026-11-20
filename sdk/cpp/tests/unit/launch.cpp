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
#include <future>
#include <iostream>
#include <sstream>
#include <string>
#include <utility>

// Third Party Imports
#include <catch2/catch_test_macros.hpp>
#include <catch2/matchers/catch_matchers_string.hpp>
#include <grpcpp/grpcpp.h>

// Local Imports
#include "bootstrap.hpp"
#include "harmos/harmos.hpp"
#include "harmos/plugin/v1/instance.grpc.pb.h"

using Catch::Matchers::ContainsSubstring;
using grpc::ClientContext;
using grpc::CreateChannel;
using grpc::InsecureChannelCredentials;
using harmos::plugin::Plugin;
using harmos::plugin::v1::Instance;
using harmos::plugin::v1::InstanceStopRequest;
using harmos::plugin::v1::InstanceStopResponse;
using harmos::tests::Bootstrap;
using harmos::tests::set_environment;
using std::async;
using std::launch;
using std::ostringstream;
using std::pair;
using std::stoi;
using std::string;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

TEST_CASE("A Managed Run Announces Once and Writes Nothing") {
  Bootstrap bootstrap;
  ostringstream output;
  auto *original = std::cout.rdbuf(output.rdbuf());
  try {
    auto serving = async(launch::async, [] {
      Plugin("example", "1.0.0").run();
    });
    const auto announce = bootstrap.announced();
    REQUIRE(announce.token() == Bootstrap::token);
    const string loopback = "127.0.0.1:";
    REQUIRE(announce.address().starts_with(loopback));
    REQUIRE(stoi(announce.address().substr(loopback.size())) > 0);
    auto instance = Instance::NewStub(CreateChannel(announce.address(), InsecureChannelCredentials()));
    ClientContext context;
    InstanceStopRequest stop;
    stop.set_reason("Story Stop");
    InstanceStopResponse stopped;
    REQUIRE(instance->Stop(&context, stop, &stopped).ok());
    serving.get();
  } catch (...) {
    std::cout.rdbuf(original);
    throw;
  }
  std::cout.rdbuf(original);
  REQUIRE(output.str().empty());
  REQUIRE(bootstrap.count() == 1);
}

TEST_CASE("A Refused Announce Fails the Start With the Host Message") {
  Bootstrap bootstrap("Launch Credential Is Invalid");
  REQUIRE_THROWS_WITH(Plugin("example", "1.0.0").run(), ContainsSubstring("Launch Credential Is Invalid"));
}

TEST_CASE("One Launch Variable Is a Start Error Naming the Other") {
  for (const auto &[present, missing] :
       {pair{"HARMOS_HOST_ENDPOINT", "HARMOS_LAUNCH_TOKEN"}, pair{"HARMOS_LAUNCH_TOKEN", "HARMOS_HOST_ENDPOINT"}}) {
    set_environment(present, "127.0.0.1:9");
    set_environment(missing, nullptr);
    CHECK_THROWS_WITH(Plugin("example", "1.0.0").run(), ContainsSubstring(string("Set ") + missing));
    set_environment(present, nullptr);
  }
}

/* === Private Functions === */

/* === Entry Point === */
