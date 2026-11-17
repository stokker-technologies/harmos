# Idle Stream Deadlines

## Why

Active transfers must survive total durations longer than policy timeout.

## What Changes

Use stream movement to rearm operation timeout, retain total deadlines for unary work, ignore transport total caps for streaming operations, and document FOLLOW heartbeats. No policy fields or size limits change.

## Impact

Harmos SDK stream endpoints, runtime execution and gRPC ingress; a product adopts the merged revision in ../product.wt/idle-deadlines. Harmos delivers first.
