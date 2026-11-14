// SPDX-License-Identifier: Apache-2.0
#include "conformance.hpp"
#include <grpc/event_engine/event_engine.h>
#include <iostream>

namespace fixture_types = harmos::conformance;
using grpc_event_engine::experimental::ShutdownDefaultEventEngine;
using std::exception;
using std::invalid_argument;
using std::string;

int install_itself(int argc, char **argv);

int main(int argc, char **argv) {
  int result = 0;
  try {
    if (argc > 1 && string(argv[1]) == "--install-itself") {
      result = install_itself(argc, argv);
    } else {
      auto plugin = fixture_types::plugin();
      if (argc == 3 && string(argv[1]) == "--address")
        plugin.serve(argv[2]);
      else if (argc == 1)
        plugin.serve();
      else
        throw invalid_argument("Usage: harmos-cpp-conformance [--address ADDRESS]");
    }
  } catch (const exception &error) {
    std::cerr << error.what() << std::endl;
    result = 1;
  }
  // Drain EventEngine Work Before Windows Begins DLL Teardown.
  ShutdownDefaultEventEngine();
  return result;
}
