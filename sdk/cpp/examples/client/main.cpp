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
#include <iostream>

// Third Party Imports

// Local Imports
#include "harmos/harmos.hpp"

using harmos::Client;
using std::exception;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

/* === Private Functions === */

/* === Entry Point === */

int main(int argc, char **argv) {
  if (argc != 2) {
    std::cerr << "Usage: harmos-cpp-client HOST:PORT\n";
    return 2;
  }
  try {
    auto client = Client::connect(argv[1]);
    const auto status = client.status.get();
    std::cout << status.status().identity().application_name() << '\n';
    return 0;
  } catch (const exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
