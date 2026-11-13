# Tooling Design

## C++ Build and Consumption

The 2026-11-13 owner decision replaces in-build gRPC with required Protobuf and gRPC config packages. The vcpkg manifest and bootstrap pin the 2026.05.25 registry commit `d015e31e90838a4c9dfa3eed45979bc70d9357fc`, providing gRPC 1.76.0#1 and protobuf 6.33.4#2. Mise has no cross-platform vcpkg registry tool, so the C++ prepare operation bootstraps that checkout in the machine tool cache. Native vcpkg binary caching is retained locally and persisted in CI; compiler-cache configuration remains machine-owned.

Ninja configure, build and test presets own development, release and test configurations. One protocol function uses packaged `protobuf_generate()` with `allow_sync_server_api=false,allow_cq_api=false`; explicit inventories replace protocol globs. Targets own C++20 and warning options. Tests retain their existing runner and case list because the runner has no listing operation.

Client fixture protocols retain full server generation through the same function's `SYNC_SERVER` argument because the existing client-root tests implement synchronous fixture services; SDK and conformance protocols retain callback-only server generation.

The installed package exports `Harmos::Cpp`, discovers its dependencies and includes authoring, client and generated protocol headers. The generated client moves to `include/harmos/generated/client.hpp`; the existing generated test client moves into the build's generated test directory. The conformance constructor and implementation are example support, compiled against the installed SDK by the package-isolation story. Python stories and their host helper retain their current locations because their relocation would also change imports.

## SDK Tools and Ownership

Only this session writes the locked inventory: `sdk/cpp/**`; C# and Python SDK mise configuration; root mise configuration and locks; SDK build/test/package/generation tooling in `tools/python/src/harmos_tools/**`; affected GitHub workflows/actions; maintained guides; and this OpenSpec change. Protocol inputs, Rust crates, and SDK public behavior remain unchanged. README links and root commands remain valid; detailed prerequisite changes belong in the linked guide.

Each SDK folder declares its language tools and build, test, format and check operations. Root SDK selections delegate into that folder. Python reuses root Python and uv plus the already locked package environments. C# filters its existing xUnit unit cases; Python's unit task selects the existing self-contained client cases without moving SDK source. Python needs no additional mise binaries beyond root Python and uv, so it retains its package lock rather than adding an empty mise lock. Nested tool locks retain the repository's version 3 format. SDK CI producers install their declared tools; jobs consuming compiled fixtures do not install CMake or dotnet. Local commits are the delivery boundary; no push or specification lifecycle transition is authorized.

Focused Rust test tooling supplies the C++ preset artifact through the existing `HARMOS_CPP_PLUGIN` override; Rust crate sources and their local fallback paths remain untouched. Two tooling tests outside the assigned inventory still assert the replaced root SDK commands; their expectation updates require ownership authorization.

The repository Python integration suite includes a real C# package consumer and a Rust crate consumer. Its task runs inside the C# mise environment; the Python check job installs C# and Rust only for that matrix entry. The generated-Python-only entry installs Python and uv.

Compiled C# fixtures still require the pinned .NET runtime: setup activates `DOTNET_ROOT` for Rust fixture consumers in Test and Coverage without installing C++ build tools there. Non-check jobs omit repository lint tools and unrelated runtimes.
