// SPDX-License-Identifier: Apache-2.0
#include <iostream>
#include "harmos/conformance.hpp"

int install_itself(int argc, char** argv);

int main(int argc, char** argv) {
    try {
        if (argc > 1 && std::string(argv[1]) == "--install-itself") return install_itself(argc, argv);
        auto plugin = harmos::conformance::plugin();
        if (argc == 3 && std::string(argv[1]) == "--address") plugin.serve(argv[2]);
        else if (argc == 1) plugin.serve();
        else throw std::invalid_argument("Usage: harmos-cpp-conformance [--address ADDRESS]");
        return 0;
    } catch (const std::exception& error) {
        std::cerr << error.what() << std::endl;
        return 1;
    }
}
