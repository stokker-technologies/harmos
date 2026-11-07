# Native SDK ABI Design

## Locked Direction

Rust owns contracts, schema validation, Postcard, operation lifecycle and framed
transport. C++ reflects fields to an owned Harmos Schema and catches exceptions.
FFI owns marshaling, opaque handles, allocation pairs and panic containment only.
Existing typed Rust registrations and transport bytes remain compatible.

## Implementation

Add encoded registration to the SDK wire table using its existing prepared work
and cancellation lifecycle. Sidecar dynamic registration composes that table and
the existing lifecycle actions. Shared JSON/Postcard conversion belongs to
harmos-domain; Dokime may adopt it later without changes in this prototype.

Runtime stream descriptors feed the existing publication builder. Arrow batches
move through the C Data Interface without copying buffers at the ABI boundary;
the framed process boundary serializes Arrow IPC inside an ordinary publication.
The host therefore retains normal stream ordering and admission. This is not
zero-copy interprocess transport. Stream revisions are derived by the host from
schema and window, matching dev; no authored stream revisions enter the ABI,
C++ or inert declarations.

The ABI adds stream declaration and callback-output allocation functions to the
suggested shape, plus a field-tag query delegated to the domain (the Rust macros
use the same const function): declarations must precede host discovery, and returned bytes
must have an explicit allocator. C++ callbacks run on Rust-owned worker threads,
borrow their context only until return, and return failure instead of unwinding.
Plugin free follows serve and settlement; it never races live callbacks.

CMake pins nanoarrow and a lightweight JSON library by version and SHA-256.
The prototype links with MSVC reference elimination enabled: otherwise unused
const metadata encoders retain incomplete marker prefixes in a foreign executable.
The exporter's marker family includes a terminator in its stored literal so linker
pooling cannot accidentally create an empty full marker. The host inspector stays
strict. Build-time inert metadata is generated from the runtime catalog for the existing
host executable inspector; no author writes fingerprints or transport controls.

## Proof and Limits

One focused Rust host story launches the C++ process and proves contracts, typed
response, nested operation, Arrow values, cooperative cancellation, exception
failure and physical shutdown. A panic test covers the ABI barrier. The example
blocks on the Rust core's cancellation signal through a panic-contained ABI wait.
The owner-selected gate runs build:cpp and test:rust:cpp with eight Cargo jobs,
lowered only on allocation failure. No full lanes, lint tasks or simulations run.
Ordinary Rust builds and CI do not need a C++ compiler; C++ proof remains opt-in.
The ffi crate has staticlib and rlib library targets and remains deliberately unpublished.

## Remaining Proof and API Review

Linux and throughput remain unproven in this delivery. Future API review should
address nanoarrow construction verbosity and ctx.operation's reach, currently
limited to this plugin's registered operations. No API redesign is included.
