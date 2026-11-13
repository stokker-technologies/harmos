# Tasks

## 10. Locked Typed Python SDK

- [x] 10.1 Generate typed Python resource classes and composed fixture roots; replace runtime binding and migrate caller-control stories.
- [x] 10.2 Ship typed package markers, annotate SDK/tooling/tests, enable annotation/import/naming rules and repair type findings without blanket ignores.
- [x] 10.3 Add annotated client and plugin examples, real-fixture stories, Python CI coverage and maintained guide contracts.
- [x] 10.4 Derive all four language roots from descriptors in memory; remove serialized resource inventories and compare fresh generation with committed roots.
- [ ] 10.5 Commit the complete source and run the requested format, Python/repository checks, SDK units/integration/example stories, generation drift and tooling tests on the committed head; verify clean status and local-only commits.

## Workflow Follow-Up

- The shared root Python Ruff invocation also checks C++ scripts. Its new ANN rules expose missing return annotations in `sdk/cpp/tests/integration/test_managed_plugin_passes_every_rule.py` and `sdk/cpp/tests/integration/test_packaged_conformance.py`. These files are outside the locked Python ownership. Root Python proof remains open pending the owner's decision on the two annotations; no rule exemptions or check weakening are introduced.

## 9. Locked C# Build Modernization

- [x] 9.1 Repair Python remote conformance host selection, commit, and prove Python checks and both C++ stories retain 31 rules.
- [x] 9.2 Move all C# projects into the solution layout; centralize properties, package versions, locked restore and artifacts output.
- [x] 9.3 Replace project loops with solution tasks, migrate generated roots and drift checks, and migrate every C# output consumer.
- [x] 9.4 Complete NuGet metadata, repository-derived version, SourceLink and deterministic CI builds; prove an independent local-feed consumer.
- [x] 9.5 Add native Linux/Windows C# check/test lanes and code analysis; update affected maintained guides.
- [x] 9.6 Run the requested committed-head proof from clean C# artifacts, root format, Python/repository checks and verify clean status; retain local commits only.

## 1. Contract

- [x] 1.1 Fix the final entry-point arguments and workflow job list in proposal.md against the implemented dispatcher.
- [x] 1.2 Lock mise.toml, tools/python, workflow/action files and contributor guidance as the source inventory.

## 2. Tasks and Tooling

- [x] 2.1 Replace the task families with the entry points, with language and layer as arguments, and delete the tasks for removed facilities.
- [x] 2.2 Remove every `CARGO_INCREMENTAL` setting from `mise.toml` and tooling and set it once in the workflows.
- [x] 2.3 Remove storage preflight and its wrapper from this project-owned tooling copy; keep paired project interfaces harmonized.
- [x] 2.4 Prune the compatibility and extended selections to what still proves something.

- [x] 2.5 Align publication --dry/--stage and build --artifact fixtures with the paired interface.
- [x] 2.6 Accept --features on package-focused build/test commands and isolate Python SDK environments.
- [x] 2.7 Update the uv pin to 0.13.6 and declare protocol-generated source paths.

## 3. Workflows

- [x] 3.1 Move archiving, summaries, the Nightly freshness check and report, the release review and the mirror push into tasks.
- [x] 3.2 Rewrite the CI, Nightly and Release workflows as matrix job templates that each run one task.

## 4. Final Validation

- [ ] 4.1 Run every entry point locally on Windows and Linux, and confirm hosted CI and Nightly run the same tasks.
- [x] 4.2 Update the contributor guide.
- [ ] 4.3 Verify the installed repository checker accepts the entry-point task model and generated protocol declarations; hand remaining checker defects to its owner.
- [x] 4.4 Prove the Committed C++ Presets from Clean Output, the Installed Consumer and Integration Stories on Windows; Record Cold and Cached Build Durations in the Final Report.
- [x] 4.5 Run Root Format and Repository Checks, Per-SDK C# and Python Build/Unit Tasks, and Verify Clean Git Status on the Committed Candidate.
- [x] 4.6 After Ownership Authorization, Update the Two Tooling Dispatcher Expectations in `tools/python/tests/integration/test_build_cargo.py` and Rerun `mise run check -- --language python`.

## 5. Developer Build Feedback

- [x] 5.1 Measure focused clean and incremental check/build/test selections and the Tests producer before tuning; own build configuration, Cargo profile/dependency sections, `mise.toml`, `tools/python` build/task tooling and its tests, and `docs/tooling.md`.
- [x] 5.2 Rank measured bottlenecks and improve configuration, dependency reuse and task orchestration without changing product behavior, release outputs or required checks.
- [x] 5.3 Repeat the measurements, evaluate independent build concurrency and document the supported workstation layout and unmeasured platforms in `docs/tooling.md`.
- [x] 5.4 Run formatting, Rust/Python/repository checks and the Tests producer; repair owned failures and report external blockers precisely.

## 6. C++ SDK Build

- [x] 6.1 Replace Fetched gRPC with Pinned vcpkg Config Packages, Packaged Generators and Native Binary Caching; Bootstrap vcpkg Outside the Repository.
- [x] 6.2 Split Protocol Generation, Tests and Conformance Targets; Add Ninja Presets, Target-Owned Settings and Installed Harmos Package Exports.
- [x] 6.3 Move Generated Client Headers and Conformance Support, Update Generators and Includes Without Changing SDK Behavior; Leave Installed-Consumer Execution to Final Validation.
- [x] 6.4 Update SDK Packaging, Artifact Consumers, Tooling and C++ Guide Contracts; Commit the Complete Dependency/Build Step Locally.

## 7. Per-SDK Tools

- [x] 7.1 Add C++, C# and Python Mise Tools, Tasks and Locks; Remove SDK Tools and Compiler-Cache Variables from Root Configuration and Delegate Root SDK Tasks.
- [x] 7.2 Install Only Lane Tools in GitHub Jobs and Persist the C++ Binary Cache; Retain All Existing Checks and Tests.
- [x] 7.3 Update Maintained Tooling Guidance and Commit the Per-SDK Configuration Step Locally.

## Protocol Skeleton and Contract Comments

- [x] P.1 Own `proto/harmos/**` layout/comments, root Buf configuration and mise entries (including the generated Buf lock entry), comment/layout-derived generated outputs, `docs/architecture.md` and this change. Derive contract comments from handlers, SDK consumers, stories and design contracts; record unresolved semantics narrowly in proposal.md.
- [x] P.2 Add Buf formatting and lint with documented grammar exceptions, excluding application/third-party inputs; apply the complete symmetric skeleton to the 13 files requiring no message reordering and retain the locked proto2 settings grammar.
- [x] P.3 Regenerate committed protocol outputs through `generate`, review README impact and document the maintained protocol authoring convention.
- [x] P.4 On the committed candidate, prove descriptor equivalence against the baseline, run format, repository and Rust checks, the focused generated-code drift test and protocol Cargo check, and leave a clean local commit without pushing.
- [x] P.5 Resolve the raw-descriptor versus Buf import/declaration ordering conflict with the owner, then apply the remaining skeleton/pairing to `v1/{invocations,plugins,status,storage_types,time}.proto`, `v1/access/{listener,pairings}.proto`, `v1/status/logs.proto`, `plugin/v1/instance.proto` and `context/v1/{invocations,time}.proto`. All declarations and file boundaries remain locked.

## Workflow Follow-Up

- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.

## Windows Build Measurements

Ryzen 9 5950X, 80 GiB RAM, Rust 1.99.1; other builds were active. Cold targets
retained the shared compiler cache. Warm probes added and removed an unused
constant, then built the runtime consumer; rows average edit and restore runs.
These observations overlap cache/profile effects and are not isolated guarantees.

| Selection | Before | After |
| --- | --- | --- |
| Warm Runtime Check / Build / Library Test | 8.9 / 15.1 / 36.6 s | 4.7 / 21.1 / 14.7 s |
| Warm Operations Check / Build / Library Test | 13.4 / 23.1 / 36.4 s | 7.0 / 13.0 / 21.5 s |
| Warm Protocol Check / Build / Library Test | 20.7 / 26.2 / 35.6 s | 15.5 / 22.6 / 36.7 s |
| Empty-Target Runtime Build | 186 s | 110 s |
| First Example Selection | 802 s | 183 s |
| Tests Compilation and Archive | 576.3 s | 493.4 s |
| Separate Fixture Driver | 167.9 s | <0.001 s |
| Guest Fixture | 210.2 s | 43.7 s |
| Offline Inputs | 24.6 s | 32.4 s |
| Complete Cold Tests Producer | 998.4 s | 590.8 s |
| Complete Warm Tests Producer | 46.3 s | 41.5 s |

The cold Tests comparison used the then-canonical 32 targets and features;
the final run removed native and Wasm development outputs first. Runtime `--lib`
edit/restore took 7.0–8.5 s. An independent empty-target protocol check took 499 s
with server-side compilation versus 80–97 s client-side; wrapper waiting appeared
inside frontend timing bars. An isolated cross-drive cache probe improved from
0 hits / 70 misses to 63 hits / 7 misses, reducing the second build from 64 to 45 s;
retained-target edit/restore then took 6.6 / 4.3 s.

| Builders, Eight Jobs Each | Warm Batch Time | Relative Throughput |
| --- | --- | --- |
| 1 | 24.6 s | 1.0× |
| 2 | 25.0 s | 2.0× |
| 4 | 28.8 s | 3.4× |

Independent targets shared the cache. Coarse sampling observed up to four rustc
processes and about 3.2 logical cores of compiler work. LLD edit/restore took
31.8 / 17.3 s versus MSVC's 13.6 / 15.9 s; the default linker remains. Warm runtime
frontends took roughly 2–5 s, with under one second of code generation. Packaging's
runtime dependency still causes distinct normal/test runtime units. Linux timings,
other SDK timings, actual branch switching, more than four builders and controlled
cold-build concurrency remain unmeasured.

- [x] 5.5 Merge build-speed tooling with SDK guest profiles and focused test selectors;
  retain benchmark fixture separation and move measurements out of the usage guide.

## 8. Locked SDK Test Migration

- [x] 8.1 Align dispatcher expectations with root-only preparation and per-SDK test delegation.
- [x] 8.2 Replace the five C++ cases and runner with Catch2 v3 and automatic discovery.
- [x] 8.3 Move shared client inputs under SDK tests and compose real library roots in all four SDKs.
- [x] 8.4 Move the unchanged conformance protocol to test inputs and exclude generated fixtures from shipping packages.

Final proof runs on the committed candidate: root format; Rust, Python and repository
checks; C++ configure/build/CTest presets; the Rust generated-protocol test; C# tests;
and Python client tests. The tree must remain clean, commits local and C: above 40 GB.
Windows-only local evidence does not close the existing cross-platform/hosted task 4.1.

## 11. Locked Example and Native Modules

- [x] 11.1 Move the unchanged wire contract to `examples/schematic/proto/`; own generation and drift proof in `examples/schematic/protocol`, remove library-generator example ownership, include it in Buf, and update `docs/schematic.md` and `docs/examples.md`.
- [x] 11.2 Verify native consumers and the independent Wasm guest; remove the operations runtime feature, all its gates, and dependent feature selections.
- [x] 11.3 Replace private catalog references with the existing public catalog composition API, preserving membership and names.
- [ ] 11.4 Apply imported Rust names in touched files and the default Tokio runtime in SDK/example entry points; review README impact.
- [ ] 11.5 On the committed source head, run root format, Rust/repository checks, native and Wasm builds, focused example stories, library/example protocol drift and SDK examples check; verify clean status and local-only commits.

Ownership is the owner-locked example/protocol/generator/Buf/guides inventory.
Implementation follows steps 11.1–11.4 in order, one local commit each. Final proof
remains separate; the implementing marker and unrelated tasks remain unchanged.
