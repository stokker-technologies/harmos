# Tooling Design

## Locked C# Build Modernization

The 2026-11-13 owner locks `sdk/csharp/**`, C# Python build/test/generation/package tooling and tests, Rust fixture producer C# lines, root C# delegation, validation workflow C# lanes, `.editorconfig`, `.gitignore` and maintained guides. Public package `Harmos` and public namespaces remain unchanged; commits stay local and the implementing marker remains untouched.

Before C# migration, repair Python remote conformance host selection: rule 31 (Message Ceiling Independent of Transfer Chunks) was absent because an older Product export shadowed the current focused host, not because the relocated fixture lost protocol content. Explicit host overrides remain first; local Python stories prefer the focused host and otherwise consume the Product export. Every story retains all 31 rules.

The solution owns all three projects under `src/Harmos`, `tests/Harmos.Tests` and `tests/Harmos.Conformance`. Central props own paths, framework, warnings, metadata and the version read from Cargo workspace metadata; central package management owns dependency versions. Use native artifacts output and query MSBuild for executable paths. Solution tasks own locked build, test, format/check and pack; generated client roots are committed and checked without rewriting them. Generation alone updates those roots. Use the simplest native MSBuild/CLI mechanisms and no compatibility shims.

Native `Directory.Build.targets` records SDK-relative executable paths under `artifacts/paths/<Configuration>/<Project>.txt`, allowing artifacts to move between CI jobs. The C# fixture staging function is shared by the ordinary Rust Tests producer and its focused task. The independent package consumer asks MSBuild for `TargetPath`. SourceLink comes from the pinned .NET SDK; portable symbol packages carry its mappings. The NuGet version changes from `0.1.0` to workspace `0.1.14`. Dependency updates alone use an explicit unlocked force-evaluate restore; ordinary tasks remain locked. Windows tasks invoke the pinned `DOTNET_ROOT` host because native task shell PATH can select the SDK-less system host. Check compares rendered clients in memory; it never repairs maintained sources. Default latest analyzer families remain enabled without suppressions or expanded opt-in families.

README review found its product claims and links unchanged; `docs/tooling.md` owns the new local-feed migration and corrects generated-source ownership. Validation adds C# check/test matrices without changing existing job selections: checks build the solution, tests consume uploaded C# binaries/manifests and the existing Rust fixture archive. Only Python, uv and dotnet are installed for these lanes. C++ step-zero proof explicitly selects the focused host with `HARMOS_CONFORMANCE_HOST`, because its unchanged helpers still prefer the older Product export.


## C++ Build and Consumption

The 2026-11-13 owner decision replaces in-build gRPC with required Protobuf and gRPC config packages. The vcpkg manifest and bootstrap pin the 2026.05.25 registry commit `d015e31e90838a4c9dfa3eed45979bc70d9357fc`, providing gRPC 1.76.0#1 and protobuf 6.33.4#2. Mise has no cross-platform vcpkg registry tool, so the C++ prepare operation bootstraps that checkout in the machine tool cache. Native vcpkg binary caching is retained locally and persisted in CI; compiler-cache configuration remains machine-owned.

Ninja configure, build and test presets own development, release and test configurations. One protocol function uses packaged `protobuf_generate()` with `allow_sync_server_api=false,allow_cq_api=false`; explicit inventories replace protocol globs. Targets own C++20 and warning options. Tests use Catch2 v3, `Catch2::Catch2WithMain` and `catch_discover_tests()`; case names live only in the tests.

Client fixture protocols retain full server generation through the same function's `SYNC_SERVER` argument because the existing client-root tests implement synchronous fixture services; SDK and conformance protocols retain callback-only server generation.

The installed package exports `Harmos::Cpp`, discovers its dependencies and includes authoring, client and generated protocol headers. The generated client moves to `include/harmos/generated/client.hpp`; the existing generated test client moves into the build's generated test directory. The conformance constructor and implementation are example support, compiled against the installed SDK by the package-isolation story. Python stories and their host helper retain their current locations because their relocation would also change imports.

## SDK Tools and Ownership

Only this session writes the locked inventory: `sdk/cpp/**`; C# and Python SDK mise configuration; root mise configuration and locks; SDK build/test/package/generation tooling in `tools/python/src/harmos_tools/**`; affected GitHub workflows/actions; maintained guides; and this OpenSpec change. The locked follow-up also owns fixture protocol relocation, Rust consumers and removal of test-only conformance exports; product protocol behavior remains unchanged. README links and root commands remain valid; detailed prerequisite changes belong in the linked guide.

Each SDK folder declares its language tools and build, test, format and check operations. Root SDK selections delegate into that folder. Python reuses root Python and uv plus the already locked package environments. C# filters its existing xUnit unit cases; Python's unit task selects the existing self-contained client cases without moving SDK source. Python needs no additional mise binaries beyond root Python and uv, so it retains its package lock rather than adding an empty mise lock. Nested tool locks retain the repository's version 3 format. SDK CI producers install their declared tools; jobs consuming compiled fixtures do not install CMake or dotnet. Local commits are the delivery boundary; no push or specification lifecycle transition is authorized.

Focused Rust test tooling supplies the C++ preset artifact through the existing `HARMOS_CPP_PLUGIN` override; C++ local fallback paths remain unchanged; C# local consumers now read the MSBuild executable manifest under the locked follow-up. The owner extended this inventory on 2026-11-13 to the tooling expectations, protocol fixture inputs, generated bindings and their consumers across all four SDKs. Root preparation installs the root Rust toolchain; SDK preparation and tests delegate to their SDK tasks. Dispatcher tests assert this boundary and preserve exact/profile selection and environment isolation.

The repository Python integration suite includes a real C# package consumer and a Rust crate consumer. Its task runs inside the C# mise environment; the Python check job installs C# and Rust only for that matrix entry. The generated-Python-only entry installs Python and uv.

Compiled C# fixtures still require the pinned .NET runtime: setup activates `DOTNET_ROOT` for Rust fixture consumers in Test and Coverage without installing C++ build tools there. Non-check jobs omit repository lint tools and unrelated runtimes.

## Locked Test Migration

The 2026-11-13 owner decision uses Catch2 v3 with its main and CMake discovery, preserving all five existing journeys and assertions. Shared client input belongs in `sdk/tests/fixtures/client/`; only the two application protocols remain invented, composed with the real library client. Conformance input belongs in `sdk/tests/fixtures/conformance/proto/`, unchanged in content and package, and its generated code belongs only to test/conformance targets. This expands source ownership to every affected protocol generator, SDK consumer and guide; the existing implementing marker remains unchanged. Local commits only; no push or lifecycle transition.

The composed fixture uses real Status/Diagnostics roots plus application Jobs/Progress. Its Python resource classes are generated from the merged descriptors; collision and wrong-face rejection belong to generation.

Conformance implementations move with their generated messages out of shipped SDK libraries: Rust test fixtures, C# Conformance executable support and Python test fixtures. Package-isolation stories copy this support into their independent consumer, compile or load it against only the packaged SDK, and retain the complete conformance suite. C++ conformance sources carry their test input beside the example, outside installed headers and libraries.

The Python SDK test task sets its selected profile inside the Python invocation: mise renders usage values in commands, while task environment templates are evaluated before usage is available. Exact selection and project-local execution are unchanged.

## Locked Typed Python SDK

The 2026-11-13 owner locks `sdk/python/**`, the maintained Python tooling and Python SDK generators under `tools/python/**`, Python mise delegation and CI steps, maintained guides and this change. Other language generators may change only their descriptor inputs. Wire behavior, protocol inputs, Rust crates and other SDK implementations remain unchanged. This session is the only writer; commits stay local and the implementing marker remains untouched.

Generated Python resource classes own explicit request and response types and typed nested attributes. `Client` initializes the library root. Products generate their own root from descriptors and attach its typed resources in an ordinary subclass constructor. Shared fixture generation merges library and application descriptors to exercise common parents. No runtime resource binding or serialized resource inventory remains.

Per-call metadata, credentials and timeout use the native gRPC call signature; typed unary awaitables and streaming async iterators retain native `cancel()` and `read()` controls. Cancellation therefore needs no second token mechanism. Protobuf compiler output remains the untyped boundary and is narrowed by message classes and stubs. All maintained SDK, examples, tests and tooling functions carry annotations; Ruff enables E4/E7/E9/F/I/N/UP/B/ANN and pinned ty checks each package with its own environment.

Examples are linear annotated programs with callable async entry points: the client discovers the host catalog, gets status and watches one host status update; the plugin serves one public echo operation. Stories run their entry points against real gRPC host/instance fixtures, synchronizing with events and controlled protocol completion. README review retains the existing package and product claims; the maintained Python section of `docs/tooling.md` owns usage and composition.

The maintained `docs/python-sdk.md` owns typed calls, product composition and runnable examples; `docs/tooling.md` retains repository tasks and plugin lifecycle contracts. Provider JSON callbacks distinguish object and collection responses at their decoding boundary; Cargo artifact and metadata dictionaries use explicit structural types. Shared fixture Python imports use a generator module-prefix mapping, leaving protobuf identity unchanged and avoiding import-time path mutation.

Product Python binding generation consumes the immutable staged schema using its recorded producer root, verifies schema digests and normalizes imports in its Python wrapper before recording output digests; the Rust generator and protocol behavior remain unchanged.
