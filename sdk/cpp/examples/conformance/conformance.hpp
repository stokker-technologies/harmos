/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#pragma once

/* === Imports === */

// Standard Library Imports
#include <functional>

// Third Party Imports

// Local Imports
#include "generated/types.hpp"
#include "harmos/harmos.hpp"

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Declarations === */

namespace harmos::conformance {
::harmos::plugin::Plugin plugin();
std::function<void(::harmos::plugin::Registrar &)> catalog();
} // namespace harmos::conformance
