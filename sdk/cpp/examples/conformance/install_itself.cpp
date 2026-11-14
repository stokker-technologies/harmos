/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

#include "generated/types.hpp"
#include "harmos/_generated/client.hpp"
#include "harmos/harmos.hpp"
#include <iostream>

using std::size_t;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

using Value = harmos::conformance::Value;
using harmos::Client;
using harmos::StorageTransferSettings;
using harmos::VISIBILITY_PUBLIC;
using harmos::client::PLUGIN_STATE_READY;
using harmos::client::PluginEntry;
using harmos::client::PluginPlacement;
using harmos::client::PluginValue;
using harmos::plugin::Context;
using harmos::plugin::Operation;
using harmos::plugin::Plugin;
using std::runtime_error;
using std::string;

int install_itself(int argc, char **argv) {
  if (argc == 3 && string(argv[2]) == "serve") {
    Plugin("example", "1.0.0")
        .operation(Operation<Value, Value>("example.echo", VISIBILITY_PUBLIC,
                                           [](Context &, Value request) {
                                             return request;
                                           }))
        .serve("127.0.0.1:0");
    return 0;
  }
  StorageTransferSettings transfer;
  auto client = Client::connect(argv[2], transfer);
  // Install and Start This Codebase's Running Instance
  PluginPlacement placement;
  placement.mutable_remote()->set_url(argv[3]);
  PluginValue value;
  value.set_enabled(true);
  client.plugins.create("example", placement, value);
  auto watch = client.plugins.watch({.name = "example"});
  PluginEntry ready;
  while (const auto event = watch.next()) {
    if (event->has_snapshot()) {
      for (const auto &entry : event->snapshot().entries()) {
        if (entry.has_failure()) {
          throw runtime_error("Start Installed Plugin: " + entry.failure().DebugString());
        }
        if (entry.state() == PLUGIN_STATE_READY) {
          ready = entry;
        }
      }
    } else if (event->has_change() && event->change().has_entry()) {
      const auto &entry = event->change().entry();
      if (entry.has_failure()) {
        throw runtime_error("Start Installed Plugin: " + entry.failure().DebugString());
      }
      if (entry.state() == PLUGIN_STATE_READY) {
        ready = entry;
      }
    }
    if (ready.state() == PLUGIN_STATE_READY) {
      break;
    }
  }
  if (ready.state() != PLUGIN_STATE_READY) {
    throw runtime_error("Watch Plugins: Closed Before Ready");
  }
  watch.cancel();
  // Invoke Its Public Operation Through the Host
  string operation;
  string fingerprint;
  for (const auto &contract : ready.description().declarations().operations()) {
    if (contract.id() == "example.echo") {
      operation = contract.id();
      fingerprint = contract.fingerprint();
    }
  }
  string data(static_cast<size_t>(transfer.chunk_bytes()), '\0');
  data.replace(0, 5, "hello");
  Value request;
  request.set_value(data);
  const auto reply = client.invocations.create(operation, fingerprint, request.SerializeAsString());
  Value response;
  if (!response.ParseFromString(reply)) {
    throw runtime_error("Invoke example.echo: Invalid Response");
  }
  if (response.value() != data) {
    throw runtime_error("Invoke example.echo: Payload Changed");
  }
  std::cout << "hello" << std::endl;
  client.plugins.delete_("example", ready.revision());
  return 0;
}

/* === Private Functions === */

/* === Entry Point === */
