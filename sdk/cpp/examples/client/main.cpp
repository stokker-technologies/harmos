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

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

/* === Private Functions === */

/* === Entry Point === */

int main(int argc, char** argv) {
    if (argc != 2) { std::cerr << "Usage: harmos-cpp-client HOST:PORT\n"; return 2; }
    try {
        auto client = harmos::Client::connect(argv[1]);
        grpc::ClientContext context;
        harmos::client::v1::StatusGetRequest request;
        const auto response = client.status.get(context, request);
        std::cout << response.status().identity().application_name() << '\n';
        return 0;
    } catch (const std::exception& error) { std::cerr << error.what() << '\n'; return 1; }
}
