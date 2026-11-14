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

using harmos::SettingDeclaration;
using harmos::plugin::HostOperation;
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
    SettingDeclaration setting;
    setting.set_name("enabled");
    setting.mutable_default_()->set_flag(false);
    Plugin("example", "1.0.0").setting(setting).requires_operation(HostOperation::PluginsSettingsList).serve();
    return 0;
  } catch (const exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
