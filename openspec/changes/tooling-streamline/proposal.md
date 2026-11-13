# Tooling Streamline

Status: task dispatch and workflow matrices are implemented; native and hosted proof remains open.
Each project owns its copy of the tooling and applies the same repairs independently.

## Why

Developers need a small task interface with focused selections, while CI carries only job
transport and setup. Project-owned tooling must work in a standalone checkout and preserve
local compiler reuse without machine storage checks.

## What Changes

| Task | Arguments After `--` | Behaviour |
| --- | --- | --- |
| `prepare` | None | Synchronize all locked Python groups and prepare the environment. |
| `generate` | `[--profile <dev\|release>] [--sdk python]` | Generate Python SDK protocol bindings, or Product-derived clients without the SDK selector. |
| `format` | `[--check] [--language <rust\|python\|toml\|csharp>]` | Format selected maintained languages, or check their formatting. This repository has no TypeScript task. |
| `lint` | `[--language <rust\|python\|toml>] [--extended]` | Lint selected languages; TOML has formatting checks only. Extended Rust lint covers optional compatibility configurations. |
| `check` | `[--language <rust\|python\|toml\|csharp\|cpp>] [--package harmos-sdk] [--extended] [--repository] [--generated]` | Check formatting, lint, repository configuration and selected generated drift; Python includes tooling tests and consumes generated Product clients. `--repository` or `--generated` selects only that check. |
| `build` | `[--profile <dev\|release>] [--artifact <product\|tests\|bench\|fixtures>] [--language <rust\|csharp\|cpp>] [--package <name> [--check] [--features <features>] [--example <name>]] [--compatibility] [--extended]` | Fixtures joins existing Product and Tests inputs without compiling. Product by default; Tests exports an archive; Bench uses the fixed bench profile. Package selection invokes Cargo directly. C# and C++ use explicit language selections; compatibility and extended remain opt-in. |
| `test` | `[--language <rust\|python\|csharp\|cpp>] [--layer <unit\|integration\|acceptance\|system>] [--package <name> --target <lib\|TEST_TARGET> [--features <features>]] [--exact <name>] [--coverage] [--profile <dev\|release>] [--filter <expression>] [--stress-count <count>] [--extended]` | Package selection runs one Cargo target without an archive; exact filtering belongs to that selection. Unit consumes the test archive; integration, acceptance, system and coverage consume the joined fixture bundle. Python supports focused tooling tests and harmos-sdk SDK integration. C# and C++ use language selections. |
| `bench` | `[--layer <unit\|integration\|system>] [--refresh-baseline] [--extended]` | Consume existing benchmark executables and matching fixtures; retain baseline refresh and optional feature selection. |
| `package` | `[--language <rust>] [--profile <dev\|release>]` | Assemble existing Rust Product outputs without compiling. |
| `release:prepare` | `[--review]` | Prepare version and lock changes; `--review` also validates the configured runner allowlist, commits prepared changes and opens the release review in the existing hosted flow. |
| `release:tag` | None | Tag the merged release review and fast-forward main; hosted GitHub calls own identity reporting and the exact-commit CI guard. |
| `release:rehearse` | `[--target <native\|TARGET\|all>] [--profile <dev\|release>] [--stage-only]` | Rehearse existing release packages, without publication credentials. |
| `release:publish` | `[--dry \| --stage <assets\|rust\|finalize\|nightly>]` | Default: draft assets, then finalize; hosted GitHub asset publication owns identity reporting and the exact-commit CI guard. `--dry` runs a dry publication; `--stage` selects one real publication stage. The selections are mutually exclusive. |
| `release:verify` | None | Verify published release assets and opted-in installers. |

- **A small set of entry points that need no knowledge of the languages in the project:**
  `prepare`, `format`, `lint`, `check`, `build`, `test`, `bench`, `package` and the release
  steps. Language and layer are arguments.
- **Incremental compilation is disabled by one variable set in CI** and nowhere else. The
  compiler cache stays for local builds.
- **The storage preflight is gone** from the shared build tooling; this project adopts that
  tooling and drops the wrapper code that existed only for it.
- **Tasks for removed facilities are deleted:** starter generation, collection and packaging
  through the authoring tool disappear. The new C++ SDK uses language selections, and
  generated protobuf Python SDK bindings retain explicit generation and drift checks.
- **Compatibility and extended selections are pruned** to what still proves something after the
  removals.
- **Tooling stays this project's own copy,** kept harmonized with Dokime's: the Cargo wrapper,
  test runner, release scripts, cache and artifact helpers are fixed here and there alike.
- **CI workflows call tasks.** One job template per kind of work, as a matrix over languages,
  layers and platforms. A job checks out, sets up mise, restores caches, downloads the build it
  consumes, runs one task and uploads results. Tasks own archiving, summaries, the Nightly
  freshness check and report, the release review and the mirror push. The target is about 1,400
  to 1,900 lines.

## Settled

- Tooling is duplicated between Harmos and Dokime on purpose: each project is an independent
  module with its own copy. The copies are kept harmonized, with the same task names, arguments,
  module layout and behaviour, and a fix made in one is made in the other.
- Impact planning is removed. Every pull request runs every job, and its dependency graph is
  deleted.
- The composite actions stay in each repository, reduced to the setup action. The duplication
  between products is accepted, since shared tooling no longer renders CI.

## Tooling Contracts

The task interface uses `release:publish --dry` or `--stage <assets|rust|finalize|nightly>`
and `build --artifact fixtures`, matching the paired project. Focused Rust `build` and `test`
accept `--features` with `--package`; they do not require a full test archive.
Python environments belong to their project roots; an SDK invocation must not redirect the
repository tooling environment. The pinned `uv` is 0.13.6.

Protocol-generated Rust and Python paths are declared as generated in `.stkr/project.toml`.
The reported Hub check rejection of this declaration, the missing protocol error module and
the entry-point task model remains a tooling-owner follow-up until the installed check accepts them.
Do not restore task families or edit generated bindings merely to satisfy obsolete checks.

## Protocol Contract Comments

The owner locked a comment/layout-only protocol pass on 2026-11-13. Its inventory
is `proto/harmos/**`, root `buf.yaml`, the Buf tool pin and its generated lock
entry, format/repository-check mise entries, comment/layout-derived generated
outputs, `docs/architecture.md` and this change. No SDK implementation, build
configuration, workflow, declaration identity or file boundary belongs to this pass.

Contract evidence comes from `crates/harmos-operations/src/{storage,access,status,
lifecycle,host,streams,clock}`, the matching runtime managers/ports, the domain's
storage protobuf conversions and query contracts, SDK consumers and the plugin
behavior/conformance stories. The root README's product claims and usage remain
unchanged; the architecture guide owns the protocol authoring convention.

Unverified downstream semantics are deliberately restricted to the publication
owner's interpretation of `SamplesAppendRequest.sequence`, `published_at`,
`context`, `following` and `FollowingRow.published_delta`. The implemented Append
handler validates/converts a supplied timestamp and forwards these values to
`PublicationHost`; its deterministic native-publication story proves forwarding,
not assignment defaults or interpretation of the following-row duration.

The raw descriptor-byte requirement conflicts with canonical source ordering:
Buf sorts imports, changing `FileDescriptorProto.dependency`, and pairing messages
in service order changes `FileDescriptorProto.message_type`. Source-info removal
does not remove these ordered lists. The comparison policy needs an owner decision
before applying declaration reordering or claiming final descriptor proof; names,
fields, numbers, types, options, file boundaries and service method order remain
locked regardless of that decision.

## Impact

- `mise.toml`, `tools/python`, `.github/workflows` and actions, and the contributor guide. No
  product behaviour changes.
