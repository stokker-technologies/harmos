# Cancellation Contract

## Agreed Direction

```rust
let handle = runtime.operation(request).start().await?;
let response: Response<Summary> = handle.cancel().await?;
// Implementation:
ctx.cancelled().await;
Ok(summary.into())
```

`OperationPolicy::cancellation_timeout` is an explicit duration. Zero means immediate abort and is used by all existing declarations. Cancellation starts a grace window bounded by the remaining attempt and overall deadlines. A response ready at the boundary wins. Cancellation never retries. An error or missing response after cancellation becomes typed Cancelled. Cancellation before an attempt starts remains an immediate refusal. Successful responses remain successful through cleanup; cleanup failures retain their existing ownership semantics.

The SDK owns a host-independent cancellation signal, carried in ExecutionContext. Runtime attempt scopes own signal delivery. Remote wire cancellation names the invocation and signals its active context; transport cancellation still aborts abandoned frames. Guest execution is cooperative only where the guest can yield to its host; synchronous blocked guest code remains interruptible by the host deadline.

Managed gRPC calls use a separate generated Run method per operation: a stream containing the typed request, optional input items, and cancellation, with typed output items and final response. Cancellation keeps the HTTP/2 response stream open; HTTP/2 abort retains abandonment semantics. Existing ordinary RPC shapes remain available. Generated Python and externally generated TypeScript clients use the same protobuf contract.

## Ownership And Inventory

One writer owns the clean feat/cancel-final-response tree. Implement policy and SDK first, then runtime, carriers, gRPC projection/dispatch, consumer migration and docs, followed by focused proof. No coupled repository changes or unresolved owner decisions. No push, PR, release or full CI simulation is authorized. The requested workspace check and focused carrier tests are the final validation boundary.
