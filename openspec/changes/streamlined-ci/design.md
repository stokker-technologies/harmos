# Migration Design

## Inputs
The authoritative upstream contracts are in the Hub fixes worktree at openspec/changes/machine-work-root/specs/ci-contract/spec.md and specs/ci-cache-impact/spec.md. Parent owns those decisions; product owner supplies implementation details and raises contract changes.

## Ownership
Use this product's chore/migrate-hub-ci branch and hub-ci worktree. The product agent owns authored product changes and canonical regeneration; parent owns Hub integration and final cross-product coordination. No direct dev commits.

## Sequence
Record baseline failures before edits. Migrate authored sources against the stable Hub interface, then regenerate only with the rebuilt Hub handed off by parent. Run targeted tests while implementing; once source closes run complete applicable available-local proof, repair failures, then publish and monitor every required provider platform through merge.

## Baseline
Windows is the current local platform. Ordinary candidate checks/builds/tests/package and required dry-release proof are baseline scope; nightly coverage is not implicitly included. No benchmark run requested.

The initial Windows run attempted check, build, build:tests, test, package and release:publish:dry on clean dev 8e2009e. Check and crate dry publication failed resolving Cargo through the shared mise shim. Product compiled the workspace and Schematic, then failed staging a schema path with WinError 206; the machine has LongPathsEnabled=0. Dependent tasks refused the missing Product catalog, fixture stamp or generated Python output. No behavioral lane completed. The baseline did not waive these failures.

## Source Inventory

One writer owns mise.toml, stkr.toml, Cargo.toml release-test profile, cargo-harmos publication metadata, authored fixture/preflight task references, README.md, CONTRIBUTING.md and canonical Harmos skill references. Canonical regeneration owns managed Python, workflows, agent copies and embedded starters. The cargo tool ships as raw release executables and a mise installer; library crates retain crates.io publication.

Canonical machine setup resolves the shared mise defect. Existing Codex processes load CARGO_HOME, RUSTUP_HOME and MISE_GLOBAL_CONFIG_FILE from the Windows User environment into each command process. Cargo and rustc resolve version 1.99.0 with automatic installation disabled.

The pinned Python executable declares longPathAware=true; official Windows runner image provisioning enables LongPathsEnabled. The user enabled the same machine policy after the baseline. A fresh Python process copied an actual Schematic schema through the unchanged staging helpers to a 287-character path and verified its SHA-256. Keep byte-exact receipts and prefix relocation; no product path workaround is required. Native full proof remains pending.

The newly required cargo-machete scan reports source-confirmed unused semver dependencies in macros/sidecar, postcard in runtime, and sha2/tokio in Schematic app. Remove these direct edges and refresh all three Cargo locks without changing dependency versions; no ignore metadata is justified. All affected behavior remains in final native validation scope.

## Source Closure

The combined chore/migrate-hub-ci tree is the source boundary. Native root Python workspace generation, authored fixture and acceptance consumers, hidden Windows launch policy and shared generated copies are complete. One writer owns the combined tree with no outstanding source slice. The root preserves all 16 dependency resolutions; both starters preserve all nine resolutions and their projected native offline exports pass. Canonical generation owns both embedded starter trees and all managed workflow copies. Product must rebuild whenever embedded resources change; Tests Build and acceptance then consume matching receipts and fixture stamps. Full local validation remains open. Linux proof remains provider-owned. SOURCE COMPLETE establishes source closure only.

The Actions cache-service contract requires exporting ACTIONS_CACHE_SERVICE_V2 to compiler-cache shell steps. Packaging's authored-starter acceptance helper uses the new Build/Tests and Rust unit task names. Required publication proof includes the actual generated starters rather than a separately authored approximation.

## Portability
No simulator; CI provides unavailable native platform proof. Fix baseline failures without weakening checks. Maintain public CI without private Hub dependencies or local-only paths. Preserve machine environment and mandatory hooks.

Archive receipt filenames, required inputs and selected roots use POSIX paths at their boundaries. Windows native separators must never cause scoped validation to skip a recorded hash. The native targeted proof accepts an unchanged receipt and rejects a mutation selected by a Windows-native root path; byte-exact digests and relocation remain unchanged.

## Native Python Workspace

The user authorizes a virtual repository-root pyproject.toml with tool.uv.workspace.members = ["tools/python"], root uv.lock and root .venv. Existing --project tools/python commands discover that root natively. Preserve every pinned package resolution; retire nested locks only after replacement verification, without environment redirects. Canonical Hub generation owns the corresponding starter layout. Authored fixture export must project the root workspace metadata, root lock and tools/python member together. Reopen affected source and Python/producer/consumer proof before publication.

## Hidden Windows Processes

The user requires background work and tests to avoid visible terminal windows. Authored Python launch sites use CREATE_NO_WINDOW on Windows and explicit inherited handles while preserving capture and file-redirection contracts; authored Rust noninteractive process owners apply the equivalent creation flag. The existing sidecar process owner already does so. The packaging process owner composes CreationFlags with JobObject so suspension and cancellation cannot overwrite launch policy. The canonical test runner owns a hidden native console using STARTF_USESHOWWINDOW/SW_HIDE and CREATE_NEW_CONSOLE. The starter watch cancellation story retains its process group and signal helper within that inherited hidden console, preserving CTRL_BREAK coverage and its deadlines. Shared launch policy and generated starter copies remain Hub-owned. Native probes verify hidden console visibility, inherited streams and group signaling; affected producer/consumer proof remains mandatory.

Rust configuration uses the installed component name llvm-tools. Native mise lock intentionally retains unused option variants; preserve its output rather than expanding lock normalization. Effective startup selects Rust 1.99.0 and reports zero newly installed tools. Rebuilding the entire lock can move Python standalone artifacts without changing the Python version, so retain every existing non-Rust platform checksum.

## Clean Source For Publication Proof

Cargo dry publication retains its clean-source requirement. Commit the owned source-complete candidate before that proof; publication authorization is separate from a local commit, and final validation remains unchecked until Dry succeeds. Finish and archive the specification with a subsequent candidate commit before pushing. Release command execution must preserve inherited stdout/stderr when capture is false and return both captured streams when true, without opening a visible console.

