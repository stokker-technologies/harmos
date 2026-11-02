# Cancellation Final Responses

## Why

Owner decision 2026-10-30 requires started operations to return their normal typed response after cooperative cancellation, including Dokime load summaries.

## What Changes

The consuming `cancel()` joins the final response. `request_cancel()` retains signal-only control. Authored contexts expose cancellation. Operation policy supplies the cancellation grace timeout; zero preserves immediate abort for existing operations. Cleanup and child settlement remain required. Sidecar and guest controls propagate the signal without dropping the attempt. gRPC gains a managed streaming call that carries cancellation while preserving the response channel. Harmos does not generate a TypeScript execution client; its exported protobuf contracts support external generators.

## Impact

SDK, runtime execution, native/guest wire protocol, gRPC contracts and dispatch, portable policy metadata, consumers and documentation. Request integers remain u32/u64/i32/i64; u16 is unsupported and ports use u32.
