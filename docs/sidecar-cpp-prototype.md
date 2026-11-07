# C++ Sidecar Prototype

This branch is an owner-assessment prototype. It is unpublished and has no stable
ABI promise. Rust owns schemas, contracts, Postcard, lifecycle and the existing
framed sidecar transport. C++20 translates author types and exceptions through
the Rust static library. Go and the transport migration are outside this work.

## Build and Run

Use the repository's pinned tools. Windows needs Visual Studio Build Tools 2022
with the x64 C++ compiler; the build task locates it with `vswhere`. CMake 4.1.2
is selected in `mise.toml`; Ninja is unnecessary. FetchContent verifies SHA-256
for nanoarrow 0.7.0 and nlohmann/json 3.12.0.

```powershell
mise trust
mise install --locked
$env:CARGO_BUILD_JOBS = '2'
mise run build:cpp
mise run test:rust:cpp
```

The second command builds and runs one focused Rust host story plus the ABI panic
test. It launches `target/cpp/harmos-bench.exe` on Windows or
`target/cpp/harmos-bench` on Linux through the real plugin host. A sidecar requires
the host's authenticated inherited control channel; launching it directly is
not an interactive demo. The story is the runnable demo.

`build:cpp` builds one Rust static library, generates the checked-in
[`harmos.h`](../crates/harmos-ffi/include/harmos.h) with cbindgen 0.29.2, then links
the C++ example. The focused test also installs the CMake package and builds
the standalone example through `find_package`. It retains the linker output and appends the exported inert
declarations to a separate host-inspectable executable. The host's existing
inspector validates those declarations. This adds no wire format or runtime
metadata authority. The task uses at most two Cargo jobs and builds serially.

For a CMake consumer, add `sdk/cpp` with `HARMOS_FFI_LIBRARY` pointing to the built
static library and link `Harmos::Cpp`. An installed prefix supports
`find_package(Harmos CONFIG REQUIRED)` and the same target. The prefix includes
the C++ headers, C header, Rust static library and pinned header dependencies.

## Start Reading

1. [`sdk/cpp/examples/bench/main.cpp`](../sdk/cpp/examples/bench/main.cpp): author
   requests, response types, field declarations and the linear capture flow.
2. [`sdk/cpp/include/harmos/harmos.hpp`](../sdk/cpp/include/harmos/harmos.hpp):
   reflection and the typed C++ interface.
3. [`crates/harmos-ffi/src/lib.rs`](../crates/harmos-ffi/src/lib.rs): C ownership,
   marshaling and panic barriers.
4. [`crates/harmos-sidecar/src/kit/dynamic.rs`](../crates/harmos-sidecar/src/kit/dynamic.rs):
   runtime registrations over the existing SDK plugin owner and wire table.
5. [`crates/harmos-domain/src/schema/json.rs`](../crates/harmos-domain/src/schema/json.rs):
   shared JSON/Postcard conversion. The native host story lives in
   [`tests/system/cpp`](../crates/harmos-runtime/tests/system/cpp/).

## Authoring

```cpp
harmos::Plugin plugin{"bench"};
plugin.operation<bench::ReadTemperature>();
plugin.operation<bench::Capture>();
return plugin.serve();
```

Each request supplies `id`, `policy`, `Response` and `run(Context&) const`.
`HARMOS_FIELDS` reflects up to eight fields in declaration order. Rust assigns
the same name-derived tags used by its authoring macros. Supported C++ fields
are bool, fixed-width 32/64-bit integers, float/double, string, bytes, nested
products, optional values and vectors. Enums and map convenience declarations
remain outside the C++ translation layer. The Rust JSON codec handles the
canonical schema's variant and product shapes.

`Capture::streams` declares names and retained windows before discovery.
`ctx.operation(request)` returns the declared response and travels through the
host, retaining the parent attempt. `ctx.check_cancelled()` throws on a requested
cancellation. `ctx.publish("temperature", batch)` consumes the batch's Arrow C
Data Interface structures. A name is local to the plugin; the exported stream
is `bench.temperature`. Its publication row is the shared `ArrowIpc` envelope,
so the retained window counts batches. Column schemas travel within IPC batches;
this prototype does not freeze column schemas across publications.

The driver is simulated: temperature is `20 + channel * 0.5`; capture adds
`0.25` per sample. A negative channel throws. A zero sample count publishes one
readiness batch and cooperatively waits for cancellation. There is no hardware
integration. C++ supplies a prototype default capacity of 16 and a cancellation
settlement timeout of two seconds; authors may override these consumer policies.

## ABI Behavior Contract

```c
hrm_schema_field_tag
hrm_plugin_new
hrm_plugin_operation
hrm_plugin_stream
hrm_plugin_serve
hrm_plugin_free
hrm_context_cancelled
hrm_context_operation
hrm_context_publish_arrow
hrm_buffer_copy
hrm_buffer_free
hrm_error_free
```

- Functions return zero on success and one on failure, with an optional error
  output. `new` returns a handle or null. `cancelled` returns 0, 1 or -1 for an
  invalid context or panic. `schema_field_tag` returns zero for invalid input.
- Handles are opaque. Registration and serve have exclusive access to the
  plugin. Register before serving; serve runs once. Free exactly once after
  serve and every callback have settled. No handle operation races free.
- Strings are borrowed NUL-terminated UTF-8. Byte slices are borrowed for the
  call; null is permitted only for an empty slice. Outputs start zeroed. Every
  returned buffer is allocated by Rust and freed with `hrm_buffer_free`; errors
  are freed with `hrm_error_free`. Both frees reset the value and accept empty
  values. Never free these allocations through C++ allocators.
- Callbacks execute on Rust-owned blocking threads. Their context, request and
  output slots are borrowed until return. Context methods run on that callback's
  Rust-owned thread. The callback/user pair remains valid
  until serve settles; callbacks may run concurrently. Return response JSON in
  a buffer allocated through `hrm_buffer_copy`. Return failure with an error
  message allocated through the same function. Rust consumes both outputs.
- Normal requests and responses are JSON product objects. Rust validates their
  schemas and encodes canonical Postcard bytes. Unknown fields, incorrect scalar
  types, truncation and trailing encoded data fail. Fingerprints and protocol
  controls stay in Rust.
- C++ callbacks are `noexcept` and catch standard and unknown exceptions.
  Exceptions become permanent operation failures; Rust catches panics at each
  ABI entry. Foreign callbacks and Arrow release callbacks must never unwind.
  Pointer validity and the Arrow C Data Interface validity rules remain caller
  obligations; panic barriers cannot repair invalid pointers or process aborts.
- `publish_arrow` validates non-null arguments, then moves both Arrow structures
  and clears their release pointers, including on subsequent import/publication
  failure. Rust retains their buffers through serialization and invokes their
  original release callbacks. Buffer transfer at the ABI is zero-copy. The
  existing framed process boundary serializes Arrow IPC into the ordinary
  `ArrowIpc` publication payload; interprocess transfer is not zero-copy.
- Cancellation is cooperative. A dropped attempt retains its physical callback
  until SDK stop joins it. Plugin retirement settles operations and the native
  process ending receipt before reporting cleanup complete. Existing typed Rust
  sidecars use their original registration and publication entry points.

## Prototype Scope

Unary operations and Arrow batch publications are implemented. There is no
author configuration/resource API, automatic retry, externally required operation
declaration API, operation input/output streaming, hot-registration or ABI version
negotiation. Nested calls currently target this plugin's own registered operations.
The example exercises two operations and one stream. These limits belong to this
prototype and are not shipping support claims.

The focused story covers discovery with nominal Rust contracts, a typed response,
a nested call, Arrow contents, cancellation settlement, foreign exception failure,
continued use and physical shutdown through the host's joined ending receipt.
Separate codec tests compare native Postcard and JSON conversion. Linux proof
was not run: the existing repository Linux executor/tool cache lacks pinned
CMake 4.1.2. Rust, Python, uv, sccache, g++ and make are already present there.
Installing CMake in that managed cache is the remaining prerequisite; no new
provisioning was performed. No full simulation or full test lane is required
for this branch assessment. Dokime's Python SDK adopting the shared Rust codec
is a follow-up; Dokime is unchanged.
