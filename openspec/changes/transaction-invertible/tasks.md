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
- [ ] 3.2 Align architecture/authoring skills, generated copies and product guide with plugin placement, operation directories, declaration-only gates, the orphan rule and #58 API names; review README impact.
- [ ] 3.3 Move schematic sidecar benchmark registration into a declaration-gated module; remove item-level benchmark gates.
- [ ] 3.4 Share retained/write_all capacity admission, refresh accounting, retain exact failed writes and prove cross-file capacity release and retry behavior in storage integration tests.
- [ ] 3.5 Verify arbitrary macro paths retain Requires/Dependent implementations; add targeted regression proof and repair only if needed.
- [ ] 3.6 Replace schematic story readiness guesses with a deterministic readiness signal bounded by configured policy; run at most one schematic story.
- [ ] 3.7 Reproduce the SDK grpc hashed-field span test and fix its root cause if it fails.
- [ ] 3.8 Exact-pin published sibling dependencies to the workspace version and check release synchronization/validation tooling.
- [ ] 3.9 Ensure release assets include git-cliff's generated CHANGELOG.
- [ ] 3.10 Last: audit unused dependencies, refresh all workspace dependencies including breaking releases, repair local fallout and record design-changing skips; commit dependencies separately.
- [ ] 3.11 Last: refresh every mise tool and lock; commit tools separately.

## HF1 Final Validation

- [ ] 4.1 Record targeted command outcomes and remaining defects; no full lanes, coverage, benchmarks or push.
- [ ] 4.2 Retire transaction-invertible with stkr specification finish and commit the retirement.
- [ ] 4.3 Write the requested report to the supplied output path, at most 25 lines.
