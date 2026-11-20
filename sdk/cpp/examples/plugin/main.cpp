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
#include <cstdlib>
#include <iostream>
#include <memory>
#include <stdexcept>

// Third Party Imports

// Local Imports
#include "harmos/harmos.hpp"

using harmos::SettingDeclaration;
using harmos::plugin::HostOperation;
using harmos::plugin::Plugin;
using harmos::plugin::Registrar;
using std::exception;
using std::runtime_error;
using std::unique_ptr;

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
    auto plugin = Plugin("example", "1.0.0")
                      .add([&](Registrar &ctx) {
                        ctx.add(setting);
                      })
                      .add([&](Registrar &ctx) {
                        ctx.add(HostOperation::PluginsSettingsList);
                      });
#ifdef _WIN32
    char *raw = nullptr;
    size_t length = 0;
    if (_dupenv_s(&raw, &length, "HARMOS_ADDRESS") != 0)
      throw runtime_error("Read Plugin Address: Check HARMOS_ADDRESS in the Process Environment");
    const unique_ptr<char, decltype(&std::free)> configured(raw, &std::free);
#else
    const char *configured = std::getenv("HARMOS_ADDRESS");
#endif
    if (configured) {
      plugin.run();
      return 0;
    }
    auto listener = plugin.start("127.0.0.1:0");
    std::cout << "Listening on " << listener.address() << std::endl;
    listener.wait();
    return 0;
  } catch (const exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
