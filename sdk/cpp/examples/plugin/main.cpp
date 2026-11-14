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
#include <google/protobuf/wrappers.pb.h>

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

int main() {
    using google::protobuf::StringValue;
    try {
        harmos::Operation<StringValue, StringValue> echo("example.plugin.v1.Echo/Call", harmos::wire::VISIBILITY_PUBLIC,
            [](harmos::Context&, StringValue request) { return request; });
        harmos::Plugin("example", "1.0.0").operation(echo).serve();
        return 0;
    } catch (const std::exception& error) { std::cerr << error.what() << '\n'; return 1; }
}
