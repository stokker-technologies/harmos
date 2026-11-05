# Idle Stream Deadlines

## Why

Active transfers must survive total durations longer than policy timeout.

## What Changes

Use stream movement to rearm operation timeout, retain total deadlines for unary work, ignore transport total caps for streaming operations, and document FOLLOW heartbeats. No policy fields or size limits change.

## Impact

Harmos SDK stream endpoints, runtime execution and gRPC ingress; Dokime adopts the merged revision in ../dokime.wt/idle-deadlines. Harmos delivers first.
