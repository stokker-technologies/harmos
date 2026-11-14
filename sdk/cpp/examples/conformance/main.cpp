// SPDX-License-Identifier: Apache-2.0
#include <iostream>
#include <grpc/event_engine/event_engine.h>
#include "conformance.hpp"

int install_itself(int argc, char** argv);

int main(int argc, char** argv) {
    int result = 0;
    try {
        if (argc > 1 && std::string(argv[1]) == "--install-itself") {
            result = install_itself(argc, argv);
        } else {
            auto plugin = harmos::conformance::plugin();
            if (argc == 3 && std::string(argv[1]) == "--address") plugin.serve(argv[2]);
            else if (argc == 1) plugin.serve();
            else throw std::invalid_argument("Usage: harmos-cpp-conformance [--address ADDRESS]");
        }
    } catch (const std::exception& error) {
        std::cerr << error.what() << std::endl;
        result = 1;
    }
    // Drain EventEngine Work Before Windows Begins DLL Teardown.
    grpc_event_engine::experimental::ShutdownDefaultEventEngine();
    return result;
}
