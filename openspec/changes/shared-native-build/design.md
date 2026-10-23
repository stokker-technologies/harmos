# Native Build Contract

## Ownership

The existing Harmos integration worktree owns this consumer migration. Hub owns shared generators and recipes. Wait for the validated Hub override before rendering; use stkr project update --no-provider with provider credentials disabled.

## Execution

One Build / Native job runs per selected platform/profile. Its product producer runs first; its tests producer runs only when selected consumers require test binaries. Clippy immediately precedes each compilation with matching packages, features, target and profile. Keep product and tests artifacts separate and preserve Wasm-target proof.

Mise tasks own exact feature selection. Do not duplicate feature lists in stkr.toml or make installer:dev compile tests. Test and Package consume Build artifacts independently; required tests still gate integration and publication.

## Routing And Removal

Use the existing hosted platform defaults (ubuntu-24.04 and windows-2022) for GitHub jobs. Remove all local ci.jobs runner overrides, including Build, Bench, Acceptance, Coverage and Release. Keep their tasks, platforms, profiles, deadlines and native integration gates unchanged.

## Acceptance

Generated native jobs have valid expressions even with no selected tests. Product and test behavior, Wasm target checks, packaging and required native gates remain intact. Retire superseded generated files and stale documentation. Reconcile required check names only when the validated candidate is pushed, preserving every other protection.


## Harmos Source Inventory

Authored compilation lives in tools/python/src/harmos_tools/build/fixtures.py (_compile plus product/test orchestration); generated run.py and bench.py belong to Hub templates. Preserve structured Cargo JSON capture while matching Clippy arguments and warning policy. Current mise.toml owns workspace all/no-default and four Wasm compatibility checks across the root and schematic manifests; migrate those proofs into Build. The separate examples/schematic/Cargo.toml workspace defines release-tests with the same release inheritance, disabled LTO, sixteen codegen units and disabled incremental compilation as the root workspace. Acceptance preflight consumes prepared fixtures and installs starter toolsets; inspect starter stories before changing their local routing. Keep the unrelated reduce-compile-time change untouched.

The no-default feature proof includes crates/cargo-harmos/src/commands/mod.rs: carrier-only command modules are compiled only with a carrier, and large internal command arguments use indirection so each supported feature selection remains warning-free without changing CLI syntax.

Sidecar-only runtime helpers in crates/harmos-runtime/src/supervision/instance.rs and crates/harmos-runtime/tests/support/pending.rs share their callers' plugin-sidecar feature gate. Disabled-feature builds omit unused helpers; enabled behavior and assertions remain unchanged.
