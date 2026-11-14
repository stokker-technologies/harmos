// SPDX-License-Identifier: Apache-2.0
#include "conformance.hpp"
#include <iostream>

namespace fixture_types = harmos::conformance;
using std::exception;
using std::invalid_argument;
using std::string;

int install_itself(int argc, char **argv);

int main(int argc, char **argv) {
  try {
    if (argc > 1 && string(argv[1]) == "--install-itself")
      return install_itself(argc, argv);
    auto plugin = fixture_types::plugin();
    if (argc == 3 && string(argv[1]) == "--address")
      plugin.serve(argv[2]);
    else if (argc == 1)
      plugin.serve();
    else
      throw invalid_argument("Usage: harmos-cpp-conformance [--address ADDRESS]");
    return 0;
  } catch (const exception &error) {
    std::cerr << error.what() << std::endl;
    return 1;
  }
}
