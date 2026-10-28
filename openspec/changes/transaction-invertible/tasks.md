# Tasks

## Implementation

- [x] 1.1 Replace `Invert`/`Undoable` with `Invertible<S>` and the hidden pair declaration; update the transaction attribute and compile fixtures.
- [x] 1.2 Migrate runtime tests, the schematic example and the authoring skill; render provider copies.
- [x] 1.3 Verify the `harmos-sdk` gRPC transcode hashed-field test on dev and repair it if it fails.
- [x] 1.4 Confirm the plugin layout rule in `harmos-architecture` and the stale `authoring-streamline` change.

## Final Validation

- [x] 2.1 Targeted proof: harmos-domain, harmos-macros, the schematic example and harmos-sdk with `grpc`.
- [x] 2.2 Open the dev review and enable squash auto-merge (PR #51); HF1 explicitly prohibits further pushes and broad validation.

## HF1 Implementation

- [x] 3.0 Merge origin/dev's Lifecycle/Setup design, preserve Invertible, and prove workspace compilation and targeted state behavior before continuing.
- [x] 3.1 Infer irreversibility from absence of Invertible; migrate macro dispatch, consumers, fixtures, example, docs and skills; remove superseded refusal fixtures.
- [x] 3.2 Align architecture/authoring skills, generated copies and product guide with plugin placement, operation directories, declaration-only gates, the orphan rule and #58 API names; review README impact.
- [x] 3.3 Move schematic sidecar benchmark registration into a declaration-gated module; remove item-level benchmark gates.
- [x] 3.4 Share retained/write_all capacity admission, refresh accounting, retain exact failed writes and prove cross-file capacity release and retry behavior in storage integration tests.
- [x] 3.5 Verify arbitrary macro paths retain Requires/Dependent implementations; add targeted regression proof and repair only if needed.
- [x] 3.6 Replace schematic story readiness guesses with a deterministic readiness signal bounded by configured policy; run at most one schematic story.
- [x] 3.7 Reproduce the SDK grpc hashed-field span test and fix its root cause if it fails.
- [x] 3.8 Exact-pin published sibling dependencies to the workspace version and audit release tooling; report the Hub-owned exact-pin synchronization defect under the no-sidequest limit.
- [x] 3.9 Audit release CHANGELOG packaging: skipped implementation because Hub-owned asset staging omits the generated notes file; report this shared-tooling defect under the no-sidequest limit.
- [x] 3.10 Last: audit unused dependencies, refresh all workspace dependencies including breaking releases, repair local fallout and record design-changing skips; commit dependencies separately.
- [x] 3.11 Last: refresh every mise tool and lock; commit tools separately.

## HF1 Final Validation

- [x] 4.1 Record actual targeted outcomes in the requested report, including the grpc-only compiler failure and shared release/documentation defects; no full lanes, coverage, benchmarks or push.
- [x] 4.2 Prepare specification retirement: all implementation dispositions, targeted evidence and explicit scope-limited skips are recorded; retirement and its commit follow this prerequisite inventory.
- [x] 4.3 Write the requested report to target/hf1-report.txt (no output path was supplied), at most 25 lines; update its retirement result after archival.

## Workflow Follow-Up

- Hub release preparation matches only bare previous versions and leaves exact sibling pins stale at the next version bump. Harmos manifests now use exact pins; the shared owner still needs repair before release.
- Hub asset staging publishes git-cliff notes as the provider release body but does not stage CHANGELOG.md as a checksummed download. No Hub changes are made during HF1 while its separate Dokime timing work is active.
- README review found pre-existing feature labels `plugin-guest` / `plugin-sidecar` where the facade exposes `guest` / `sidecar`, plus a schematic README table claiming separate request types. Report these unrelated documentation defects without expanding HF1.
- The initial shell inherited Dokime's UV_PROJECT_ENVIRONMENT and TMPDIR. The first prepare task touched that other environment before the overrides were cleared for all later compilation; its environment is not repaired while separate Dokime work is active.
- The additional grpc-only runtime compiler check fails on the existing ungated `pub(crate) use codec::encode_bytes` re-export in src/grpc.rs; its consumers are plugin-gated. Preserve the failure and report it under the no-sidequest rule; the workspace all-targets compiler check passes after dependency refresh.
