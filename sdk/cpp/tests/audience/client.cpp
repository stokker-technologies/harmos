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

#include "harmos/client.hpp"
/* === Macros === */

#if __has_include("harmos/plugin/v1/instance.pb.h") || __has_include("harmos/context/v1/time.pb.h")
#error Cross-Audience Context or Instance Bindings
#endif
/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

template<class T> concept Server = requires { typename T::Service; };
static_assert(!Server<harmos::v1::Invocations>);
static_assert(requires(harmos::Client client, harmos::v1::InvocationsCancelRequest request) { client.invocations.cancel(request); });
/* === Public Functions === */

/* === Private Functions === */

/* === Entry Point === */

int main() { return 0; }
