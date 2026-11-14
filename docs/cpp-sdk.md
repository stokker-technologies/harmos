# C++ SDK

`sdk/cpp` is a C++20 authoring and client library. Include `harmos/harmos.hpp`;
it includes the typed client, shared messages and plugin authoring boundary. The
[conformance example](../sdk/cpp/examples/conformance/) supplies test support.
The minimal [client](../sdk/cpp/examples/client/) connects and reads host status;
the minimal [plugin](../sdk/cpp/examples/plugin/) serves one typed echo operation.
Both are built by the presets and can also use the installed package independently.

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
mise -C sdk/cpp run format -- --check
mise -C sdk/cpp run check
mise -C sdk/cpp run build
mise -C sdk/cpp run consumer
mise -C sdk/cpp run test -- --layer unit
```

Build the matching `harmos-conformance` host separately before running
`mise -C sdk/cpp run test -- --layer integration`. Host-backed stories require
the runtime consumer migration to compile; the native SDK stories run independently.

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
`tests` builds Debug libraries, the Catch2 journeys and the conformance plugin;
`release` includes those targets in Release. All presets build the minimal client
and plugin. `HARMOS_BUILD_TESTS` defaults off
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
Native declaration tests run through CTest before the managed-plugin and installed-package
stories. Both check every reference-host case, including cleanup and successful
process exit after Stop.

The fixture is `target/cpp/tests/harmos-cpp-conformance.exe` on Windows and
`target/cpp/tests/harmos-cpp-conformance` on Linux. Without arguments it binds an
ephemeral loopback port and announces `harmos listening 127.0.0.1:PORT`.
`HARMOS_ADDRESS` overrides its bind address. `--address HOST:PORT` selects remote
lifecycle: Stop completes cleanup and leaves the server running. Both modes use
plain gRPC. Host clock waits require declaring `HostOperation::TimeSleep` and
`HostOperation::TimeGet` from `harmos::plugin` with `requires_operation`. Observers may call declared host operations, the host clock,
settings and demand-bounded Watch while answering a tick. Authors must not
start untracked background work if they require virtual-time execution.

Unit journeys use Catch2 v3 with `TEST_CASE` and `SECTION`; CTest discovers cases from the executable through `catch_discover_tests()`. No separate runner or case inventory is maintained.

## Boundary Types

Import `Client`, connection options, errors and shared messages from `harmos`.
Client response and options types have their single public path in `harmos::client`;
authoring and plugin-host types live in `harmos::plugin`. Include `harmos/harmos.hpp`.
Wire requests and transport adapters stay in private generated headers.

```cpp
using harmos::Client;

auto client = Client::connect("127.0.0.1:53124");
const auto inventory = client.operations.list();
const auto response = client.status.get();
std::cout << response.status().identity().application_name() << '\n';
```

Typed calls take required request content in declaration order, followed by a defaulted
per-call options value. For example, `client.plugins.get("example")` and
`client.plugins.list(PluginsListOptions{.at = cut})` own their requests and contexts.
Import `PluginsListOptions` from `harmos::client` when naming it. Mutations generate
retry identity when `request_id` is absent. Options can retain a shared ClientContext
for metadata, cancellation or deadline overrides. Operation policy owns the default
deadline. Streaming calls return an owned `ClientStream`; `next()` pulls one item,
and cancellation or destruction ends demand.

Exactly one response field returns its payload, including a repeated collection;
streams project each item likewise. Empty and multiple-field responses retain their
message. Status retains its status and resolved read cut. See the full list in
[SDK Call Contracts](architecture.md#sdk-call-contracts).

`client.operations.get` selects one complete operation identity. Typed schema and
stream inventories are `client.schemas.list` and `client.streams.list`.
Application-generated clients can compose their own resources beneath
`client.plugins` alongside the library's `client.plugins.settings` node.
`client.plugins.replace` takes a managed placement and the entry's expected
revision to replace a bundle in place.

Storage operations are owned in-process. A product declares its own typed wire
operations over them. `harmos/transfer.hpp` retains shared carrier sizing for
`Client::connect`, plugin admission and product transfer wrappers; it does not
declare library storage calls. Shared `File`, `Upload` and `FileReference` remain
available to those wrappers.

## Typed Settings

Declare each library-stored setting with its typed default. False flags and empty
text are selected values; an unset value discriminator is refused.

```cpp
using harmos::SettingDeclaration;
using harmos::plugin::HostOperation;
using harmos::plugin::Plugin;

SettingDeclaration setting;
setting.set_name("enabled");
setting.set_title("Enabled");
setting.set_description("Enable Sampling");
setting.mutable_default_()->set_flag(false);

Plugin plugin("example", "1.0.0");
plugin.setting(setting).requires_operation(HostOperation::SettingsList);
```

`Plugin::setting` contributes one `SettingsDeclarations` section and its reachable
types to `Declarations`. It refuses duplicate names and declarations without a name
or selected default. Value variants are flag, integer, number, text and duration.
Inside an operation or tick observer, `context.settings()` calls
`context.host().settings.list()` and returns the calling entry's complete typed
values, settings revision and issued read cut. It takes no plugin selector.

Clients read declarations and values through `client.plugins.settings.list` and
replace the complete map through `client.plugins.settings.set`. Set takes the
expected settings revision and an optional replay request ID. Settings revisions belong to
settings separately from plugin entry revisions; policies and declared settings
own deadlines and retries.

## Tick Child Calls

An observer's `Context` inherits the stream's opaque scope and declared requirements.
Child call IDs increase across all ticks on that Samples call. Responses may arrive
out of order; each `Call::get()` retains its own typed result or structured failure.
`Call::cancel()` cancels that child, and a `Watching` object requests each item with
`next()` and cancels its open Watch on destruction.

Answering a tick cancels its remaining children and retires its context. Ending the
stream wakes pending handlers and settles their workers. Child traffic before a tick,
after its answer, on device streams, or with unknown or duplicate terminal IDs refuses
the Samples RPC. Separate Samples calls and Invoke calls keep independent identities.
Device-driven publication remains on `Publisher::publish` under active stream demand.

The reported message is `Declarations`; the SDK builder is
`declarations()`. Regenerate client/plugin bindings when adopting this revision.
Guest settings use the same library entry and settings services as other placements;
see [WebAssembly Guests](plugin-manager.md#webassembly-guests).

## Formatting and Composition

The format task runs the pinned clang-format. LLVM defaults disallow short guards,
loops and blocks; commented deviations retain the repository width, permit only empty
short functions and expand lambdas. Source files import named types once at the top;
headers retain qualified names and define no namespace imports.

Product generators merge descriptors, put request and transport adapters in private
headers, and export their response types from a client barrel. The shared fixture proves
Annotations beneath library Plugins alongside the sibling Jobs.Progress root. Reuse the
SDK's request flattening and response projection when generating a product client.
