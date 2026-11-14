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
#include <string>

// Third Party Imports

// Local Imports
#include "harmos/harmos.hpp"

using harmos::Client;
using std::exception;
using std::runtime_error;
using std::string;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

/* === Private Functions === */

/* === Entry Point === */

int main(int argc, char **argv) {
  if (argc != 2 && argc != 4) {
    std::cerr << "Usage: harmos-cpp-client HOST:PORT [OPERATION ENCODED_PAYLOAD]\n";
    return 2;
  }
  try {
    auto client = Client::connect(argv[1]);
    const auto status = client.status.get();
    std::cout << status.status().identity().application_name() << '\n';
    if (argc == 4) {
      const auto operation = client.operations.get(argv[2]);
      if (!operation.contract().has_protobuf())
        throw runtime_error("Create Invocation: Protobuf Contract Required");
      const auto &fingerprint = operation.contract().fingerprint();
      const auto response = client.invocations.create(argv[2], fingerprint, string(argv[3]));
      std::cout << response;
    }
    return 0;
  } catch (const exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
