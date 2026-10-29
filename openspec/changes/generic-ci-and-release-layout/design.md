# Design

## Locked Scope

This is adoption task 2.2 of the Hub change agreed on 2026-10-29, following its
design and the assignment for `chore/render-hub-layout`. No product API, storage
version, provider protection or upstream implementation changes are introduced.

## Ownership And Inventory

The product owns `mise.toml`, `stkr.toml`, README, `CONTRIBUTING.md` and
`docs/product-guide.md`. The Hub owns files carrying its rendered marker.
The existing render supplies Unix family names, neutral variables, job graphs,
archive assembly, generated changelogs, exact-pin synchronization and credential
removal for rehearsal. These mechanisms are verified rather than duplicated.

Argument-bearing aggregate tasks use native parallel `run` groups. Literal
prerequisites stay in `depends`. Example bundles retain their package inputs
and checks but lose the retired release flag. The only declared public tool is
`cargo-harmos`, with ZIP on Windows and tar.gz on Unix, containing its executable,
LICENSE, README and generated CHANGELOG. Library crates remain on crates.io.

## Proof And Handoff

Run targeted task-form/format checks, local release dry/rehearsal and strict
OpenSpec validation. Record Hub-owned defects as blockers without changing their
rendered files or the Hub worktree. No full lanes, CI simulation or pushes.
Hosted validation, integration and upstream task 2.2 closure belong to the caller.
Before the next release, the owner registers crates.io Trusted Publishing for
`release.yml` in the `release` environment.

## Upstream Tooling Blockers

The rendered `release/run.py` rehearsal calls `dry(..., require_all=True)` and
offers no local-target selector. A native Windows checkout therefore needs Unix
packages too before archive and crate verification starts. Local-platform rehearsal
proof needs a Hub-owned resolution; product policy must retain both required
release targets. Do not manufacture foreign packages or edit rendered tooling.

The direct `crates-dry` dispatch in rendered `release/run.py` calls
`registry.rehearse_crates` without `_credential_free`; `release/policy.py`
inherits the parent environment. The Hub must apply its locked credential-free
direct registry dry-run contract there as well as in rehearsal.
