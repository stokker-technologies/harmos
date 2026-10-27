# Storage Integration

## Ownership And Admission

Use Harmos storage as the owner of admitted records, streams, objects, files,
retention and accounting. The product runtime retains the workers that keep
storage active. A viewer or transient query client does not own its producer.

Supply consumer-owned storage budgets, concurrency, admission settings and
response batch ceilings explicitly. Preserve retained allocation claims when
projecting or transporting results. Cloning or slicing shared batches does not
make their storage allocation free; avoid unaccounted intermediate collections.

Use the provided retained or consuming projection APIs when converting admitted
data. Inspect their aliasing, capacity and ownership requirements before
conversion. Refusal and cancellation must preserve the API's admitted-write,
retry and completion guarantees.

## Queries And Live Data

Direct storage APIs accept `harmos::storage::Query`, `FileRead`, `FileCapture`
and `FileFollow`; hashing also accepts `FileRead`. Operation wire requests are
separate contracts. Construct the resource input at the execution boundary.

Use the shared `Query`, `StorageCursor`, source and transform vocabulary rather
than implementing a different query engine in each interface. Ordered
transforms precede row slicing. Arrow schemas and record batches represent
analytical results whose lifetime can retain source allocations.

Distinguish a finite read from a live subscription. Record, stream and object
followers consume changes; their progress must not be confused with state
transaction history. Use supported saved-cursor notifications where appropriate,
preserving terminal closure, cancellation and durable visibility semantics.

## Filesystem Operations

`FileSource` distinguishes a stored file reference from a local filesystem
path. `ReadFile` and `HashFile` use the source and range contract; a local path
does not imply immutable content. `CaptureFile` creates independent stored
content under its declared consistency policy, without promising an atomic
snapshot of an arbitrary changing file.

`FollowFile` follows the selected source and reports data and applicable
truncation/replacement events. Following staged production is a distinct
production contract. `ListDirectory` and `WatchDirectory` use the shared
selection rules and change/rescan semantics. Do not replace these contracts
with operation-specific filesystem adapters in each interface.

Keep transfer buffers and retained file slices within their admitted lifetimes.
Verify supported platform behavior and filesystem permissions; using a shared
operation contract does not make a native filesystem available inside a guest.

## Aggregation And Signals

Reuse `Aggregate` from `harmos-domain`, also exposed through `harmos`:
`Count`, `Sum`, `Mean`, `Median`, `Min`, `Max`, `Rms`, `Std` and
`Percentile(u8)`. `harmos-storage` owns signal mathematics: `Series` is a
finished series and `Summary` holds mergeable streaming statistics. Median
and percentiles require the finished series.

The storage `fft` feature owns FFT kernels; the host facade exposes them
through `signal-fft`. Reuse those facilities instead of duplicating kernels
in presentation or transport code.
