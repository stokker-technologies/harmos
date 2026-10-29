# Storage Integration

## Ownership And Limits

Use Harmos storage as the owner of records, streams, objects, files and
retention. The product runtime retains the workers that keep storage active.
A viewer or transient query client does not own its producer.

Supply pending-write limits, query concurrency and serialized response batch
ceilings explicitly. Storage shares immutable batch allocations across clones,
slices and retries; consumers own budgets for retained results and conversion
scratch. There is no storage read-memory pool or ReadSession admission API.

Use consuming projection APIs when converting shared data. Inspect their
aliasing and ownership requirements: typed/file projections require unique
owners. Refusal and cancellation preserve admitted-write, retry and completion
guarantees. Retained writes await queue capacity through the same path as
write_all and preserve exact bytes for retry without cancelling production.

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

Opened-file reads retain the supplied filesystem object and independent caller
cursors. On Windows the handle must permit a separate read open. Keep named or
shareable delete-on-close temporary owners alive through reads and captures;
do not unlink a temporary before an independent reopen. StorageManager's
freeze_opened_file creates an immutable FrozenFile with length and SHA-256;
its read and capture_frozen_file paths retain the private named backing through
readers, blocking work, cancellation and exact capture retries. Local filesystem
errors identify the step and path while preserving the native source error.

Keep transfer buffers and retained file slices within their owning lifetimes.
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
