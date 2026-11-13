# C++ SDK

`sdk/cpp` is a C++20 authoring and client library. Include `harmos/harmos.hpp`;
it also includes the generated client at `harmos/generated/client.hpp`. The
[conformance example](../sdk/cpp/examples/conformance/) supplies test support.

```cmake
find_package(Harmos CONFIG REQUIRED)
add_executable(plugin main.cpp)
target_link_libraries(plugin PRIVATE Harmos::Cpp)
```

Install with `cmake --install target/cpp/tests --prefix <prefix>`, then put that
prefix on the consumer's `CMAKE_PREFIX_PATH`. `HarmosConfig.cmake` discovers
Protobuf, gRPC and OpenSSL; provide their config packages through the same toolchain.
The exported target carries C++20, include paths and transitive dependencies.

```sh
mise -C sdk/cpp install --locked
mise -C sdk/cpp run prepare
mise run build -- --language cpp
mise -C sdk/cpp run consumer
mise run build -- --package harmos-conformance
mise run test -- --language cpp
```

`sdk/cpp/mise.toml` pins CMake 4.1.2 and Ninja 1.13.2. Windows requires Visual
Studio 2022 C++ Build Tools and a Windows SDK; the task activates its native x64
compiler environment. Linux requires a C++20 compiler and the vcpkg system prerequisites
(`curl`, `zip`, `unzip`, `tar`, `pkg-config`, `autoconf`, `automake`, `libtool`).
The prepare operation bootstraps vcpkg 2026.05.25 at immutable commit
`d015e31e90838a4c9dfa3eed45979bc70d9357fc` under `mise cache path`/`vcpkg/d015e31e9083`,
outside the checkout. The same manifest baseline provides gRPC 1.76.0#1 and
protobuf 6.33.4#2; tests use Catch2 3.15.0. Runtime, `protoc` and `grpc_cpp_plugin` use those packages.
Dependencies are required config packages; CMake never fetches gRPC sources.
Native vcpkg binary caching reuses dependencies between builds and checkouts;
GitHub persists that cache between runs. Compiler-cache settings are workstation-owned.

`CMakePresets.json` defines Ninja configure, build and test presets `development`,
`release` and `tests`. Tasks and CI use those presets; editors can select them too.
Use `mise -C sdk/cpp run build -- --fresh` to reset CMake configuration while
retaining the installed dependencies and their binary cache.
For direct CMake calls, export `VCPKG_ROOT` to the prepared checkout and activate
the native compiler environment. `development` builds the shipping libraries;
`tests` builds Debug libraries, all five unit cases and the conformance plugin;
`release` includes those targets in Release. `HARMOS_BUILD_TESTS` defaults off
outside the test presets. Generated protocol files stay in each preset's build
output; generated fixture clients sit beside their generated fixture protocols.

Outputs stay under `target/cpp/<preset>/`, with retained `configure.log` and
`build.log`. Installed SDK headers and libraries live under its `installed/` prefix.
The source archive contains the SDK, manifest, conformance example and protocol inputs;
source consumers pass the vcpkg toolchain when configuring that tree. The installed
package story extracts that archive, builds its example with `find_package(Harmos)`
and runs every host rule. Test consumers use existing SDK and host builds;
`--layer unit` selects CTest and `--layer integration` selects the Python stories.
`HARMOS_CONFORMANCE_HOST` may select a separately built host.
For a focused host build, select `target/debug/harmos-conformance` (`.exe` on
Windows) explicitly with that variable; otherwise stories prefer the native
Product export when it exists.
Native catalog tests run through CTest before the managed-plugin and installed-package
stories. Both check every reference-host case, including cleanup and successful
process exit after Stop.

The fixture is `target/cpp/tests/harmos-cpp-conformance.exe` on Windows and
`target/cpp/tests/harmos-cpp-conformance` on Linux. Without arguments it binds an
ephemeral loopback port and announces `harmos listening 127.0.0.1:PORT`.
`HARMOS_ADDRESS` overrides its bind address. `--address HOST:PORT` selects remote
lifecycle: Stop completes cleanup and leaves the server running. Both modes use
plain gRPC. Host clock waits require declaring `harmos::SLEEP`/`harmos::GET_TIME` with
`requires_operation`; observers answer ticks and cannot wait. Authors must not
start untracked background work if they require virtual-time execution.

Unit journeys use Catch2 v3 with `TEST_CASE` and `SECTION`; CTest discovers cases from the executable through `catch_discover_tests()`. No separate runner or case inventory is maintained.
