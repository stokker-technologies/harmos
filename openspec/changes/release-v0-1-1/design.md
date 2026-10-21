# Design

## Ownership And Dependency Order

The release owner exclusively uses harmos.wt/release-rehearsal-render. Existing PR19 source belongs to this track and is integrated into dev. The single release integration branch is release/v0.1.1, generated from published dev after its native graph passes. Preserve the former fix branch and the generated candidate history.

Carry this change into the generated candidate before its final CI run. Do not append commits to the already merged PR19 branch. The human required a supported Hub CLI fix rather than manual marker editing: use specification rebind with the expected old branch to bind the same change/worktree to its release branch, preserving source. The development-profile authoring bootstrap is local only and proves no release-renderer readiness. Inspect OpenSpec tool pins and generated guidance before committing lifecycle files; preserve authored policy.

The paired consumer is dokime.wt/remote-streamline, change release-v0-1-0. Harmos registry and native publication must finish before Dokime adopts version 0.1.1. No Hub or Dokime drop source is owned here.

## Inventory And Phase

Current phase is implementing release source. Own Cargo.toml, nested package/fixture Cargo.toml and Cargo.lock files selected by release version policy, stkr.toml release/version fields, canonical replacement of legacy release tooling/workflows/changelog/provenance consumers, affected cargo-harmos scaffold copies and OpenSpec lifecycle files. No runtime, binding or example API change is planned.

Replacement AGENTS guidance supersedes the handoff's main-review/backmerge flow. The current generated release/registry.py:399 admits direct release promotion into main and release/run.py:412 requires tagging on main; neither implements the new flow. Adopt a repaired, verified published Hub renderer, with its matching source templates, rather than hand-editing generated files. Its rehearsal must refuse missing tree evidence, and Tag must preflight main ancestry before tag creation or Rust publication. Marker-rebind readiness alone is insufficient for renderer adoption.

The generated preparation updated package carriers but left nine root workspace sibling requirements at 0.1.0; align these authored requirements to 0.1.1 and regenerate matching locks. The new canonical preparation must preserve that alignment. Release notes come from git-cliff at tag time, with no tracked changelog or release manifest in the final cutover. Retire superseded generated paths through their owner and update affected README/scaffold claims.

Provider migration requires the actual RELEASE_TOKEN identity, complete rendered dev checks including Release / Dry and Release / Rehearsal, and main restricted to that identity's protected fast-forwards. Current main requires a review and 21 legacy checks, and stable tags allow the repository-admin role to bypass; neither policy may be silently weakened. Prepare the exact provider change and reserve explicit approval where replacing enabled main review/check rules exceeds existing authority. Retain signed commits, no force pushes/deletion, all native dev checks, review and auto-merge. RELEASE_TOKEN is currently absent; never expose or substitute a broad local credential as proof of the required identity policy.

Review README.md and existing usage against the versioned candidate. No new rustdoc or docs work. PR16's canonical license correction is separately reviewed and integrated after release as authorized.

After verified publication, add the authorized README header badges for dev CI, crates.io harmos, latest GitHub release and Apache-2.0, preferably with the existing license/README follow-up when coherent. No PyPI badge. These badges do not gate v0.1.1 publication. Helios badges belong to a later product follow-up outside this track.

## Acceptance And Authority

Run env -u TMPDIR mise run check before every push. After complete source accounting and writer handoff, require all configured Linux/Windows release-review checks and exact-candidate Release / Rehearsal. The release review targets dev and squash-merges with title chore(release): prepare v0.1.1. Notify the user before enabling that merge because its dev push starts Release / Tag. Verify the merged/rehearsed trees match, the tag identifies that dev commit, main equals it through a fast-forward, and registries/downloads/checksums/native install jobs pass. No separate review into main or backmerge. Never change v0.1.0. Preserve its failed draft until v0.1.1 is verified.

Release and merge authority comes from the handoff under the replacement guidance. No implementation-only hold is recorded. The actual renderer and provider identity/protection dependencies still block final validation/integration. Benchmarks are excluded. Worktree retirement is separate; no automatic deletion without fresh eligible integration evidence. No unresolved product API decisions.
