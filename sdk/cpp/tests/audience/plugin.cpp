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

// Third Party Imports

// Local Imports

#include "harmos/harmos.hpp"
#include "harmos/plugin/v1/instance.grpc.pb.h"
/* === Macros === */

#if __has_include("harmos/v1/time.pb.h") || __has_include("harmos/context/v1/time.grpc.pb.h")
#error Cross-Audience Client or Standalone Context Bindings
#endif
/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

template<class T> concept Caller = requires { typename T::Stub; };
static_assert(!Caller<harmos::plugin::v1::Instance>);
static_assert(requires { typename harmos::plugin::v1::Instance::CallbackService; });
/* === Public Functions === */

/* === Private Functions === */

/* === Entry Point === */

int main() { return 0; }
