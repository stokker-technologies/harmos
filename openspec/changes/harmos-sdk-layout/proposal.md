# Harmos SDK Layout

## Why

SDKs for several languages need one predictable home, and the interfaces for writing your
own sidecar or guest belong in Harmos.

## What Changes

- Move `crates/harmos-sdk` to `sdk/rust`, keeping its crate name.
- Reserve `sdk/<language>/` for each language's sidecar and guest interfaces.
- Update the workspace, tooling, documentation and consumers.

## Non-Goals

Behaviour or API changes.
