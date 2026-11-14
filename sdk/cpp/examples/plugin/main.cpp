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

using google::protobuf::StringValue;
using harmos::VISIBILITY_PUBLIC;
using harmos::plugin::Context;
using harmos::plugin::Operation;
using harmos::plugin::Plugin;
using std::exception;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

/* === Private Functions === */

/* === Entry Point === */

int main() {
  try {
    Operation<StringValue, StringValue> echo("example.echo", VISIBILITY_PUBLIC, [](Context &, StringValue request) {
      return request;
    });
    Plugin("example", "1.0.0").operation(echo).serve();
    return 0;
  } catch (const exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
