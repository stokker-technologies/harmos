# Tooling Design

## C++ Build and Consumption

The 2026-11-13 owner decision replaces in-build gRPC with required Protobuf and gRPC config packages. The vcpkg manifest and bootstrap pin the 2026.05.25 registry commit `d015e31e90838a4c9dfa3eed45979bc70d9357fc`, providing gRPC 1.76.0#1 and protobuf 6.33.4#2. Mise has no cross-platform vcpkg registry tool, so the C++ prepare operation bootstraps that checkout in the machine tool cache. Native vcpkg binary caching is retained locally and persisted in CI; compiler-cache configuration remains machine-owned.

Ninja configure, build and test presets own development, release and test configurations. One protocol function uses packaged `protobuf_generate()` with `allow_sync_server_api=false,allow_cq_api=false`; explicit inventories replace protocol globs. Targets own C++20 and warning options. Tests use Catch2 v3, `Catch2::Catch2WithMain` and `catch_discover_tests()`; case names live only in the tests.

Client fixture protocols retain full server generation through the same function's `SYNC_SERVER` argument because the existing client-root tests implement synchronous fixture services; SDK and conformance protocols retain callback-only server generation.

The installed package exports `Harmos::Cpp`, discovers its dependencies and includes authoring, client and generated protocol headers. The generated client moves to `include/harmos/generated/client.hpp`; the existing generated test client moves into the build's generated test directory. The conformance constructor and implementation are example support, compiled against the installed SDK by the package-isolation story. Python stories and their host helper retain their current locations because their relocation would also change imports.

## SDK Tools and Ownership

Only this session writes the locked inventory: `sdk/cpp/**`; C# and Python SDK mise configuration; root mise configuration and locks; SDK build/test/package/generation tooling in `tools/python/src/harmos_tools/**`; affected GitHub workflows/actions; maintained guides; and this OpenSpec change. Protocol inputs, Rust crates, and SDK public behavior remain unchanged. README links and root commands remain valid; detailed prerequisite changes belong in the linked guide.

Each SDK folder declares its language tools and build, test, format and check operations. Root SDK selections delegate into that folder. Python reuses root Python and uv plus the already locked package environments. C# filters its existing xUnit unit cases; Python's unit task selects the existing self-contained client cases without moving SDK source. Python needs no additional mise binaries beyond root Python and uv, so it retains its package lock rather than adding an empty mise lock. Nested tool locks retain the repository's version 3 format. SDK CI producers install their declared tools; jobs consuming compiled fixtures do not install CMake or dotnet. Local commits are the delivery boundary; no push or specification lifecycle transition is authorized.

Focused Rust test tooling supplies the C++ preset artifact through the existing `HARMOS_CPP_PLUGIN` override; Rust crate sources and their local fallback paths remain untouched. The owner extended this inventory on 2026-11-13 to the tooling expectations, protocol fixture inputs, generated bindings and their consumers across all four SDKs. Root preparation installs the root Rust toolchain; SDK preparation and tests delegate to their SDK tasks. Dispatcher tests assert this boundary and preserve exact/profile selection and environment isolation.

The repository Python integration suite includes a real C# package consumer and a Rust crate consumer. Its task runs inside the C# mise environment; the Python check job installs C# and Rust only for that matrix entry. The generated-Python-only entry installs Python and uv.

Compiled C# fixtures still require the pinned .NET runtime: setup activates `DOTNET_ROOT` for Rust fixture consumers in Test and Coverage without installing C++ build tools there. Non-check jobs omit repository lint tools and unrelated runtimes.

## Locked Test Migration

The 2026-11-13 owner decision uses Catch2 v3 with its main and CMake discovery, preserving all five existing journeys and assertions. Shared client input belongs in `sdk/tests/fixtures/client/`; only the two application protocols remain invented, composed with the real library client. Conformance input belongs in `sdk/tests/fixtures/conformance/proto/`, unchanged in content and package, and its generated code belongs only to test/conformance targets. This expands source ownership to every affected protocol generator, SDK consumer and guide; the existing implementing marker remains unchanged. Local commits only; no push or lifecycle transition.

The composed fixture uses real Status/Diagnostics roots plus application Jobs/Progress. Keep `resources.json`: the two Python client journeys load that generated table, including collision and wrong-face rejection.
