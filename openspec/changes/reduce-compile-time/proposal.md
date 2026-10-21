# Reduce Compile Time

## Why

The user approved applying the 2026-10-21 compilation findings with cold measurements and separate signed commits. Default storage and duplicated guest tooling increase compilation cost.

## What Changes

- Make storage optional only if module boundaries suffice; otherwise report refusal.
- Align guest tooling versions and remove unused guest dependencies.
- Reduce duplicate dependency families through ordinary compatible upstream bumps, without forced pins.
- Measure the runtime integration harness and apply only permitted mechanical improvements.
- Supply a Hub CI feature-selection note without editing CI.

## Impact

Scope is this Harmos workspace only. Dokime migration, structural harness splits, profile-policy changes, publication and CI edits are excluded. Preserve runtime and guest behavior. Each implemented item requires cold before/after evidence in its signed commit body.
