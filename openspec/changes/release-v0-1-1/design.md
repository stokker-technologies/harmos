# Design

## Ownership And Dependency Order

The release owner exclusively uses harmos.wt/release-rehearsal-render. PR19 is integrated into dev. The single release integration branch is release/v0.1.1, generated from published dev after its native graph passed. Preserve the former fix branch and the generated candidate history; the linear-pipeline adoption that briefly sat on this branch is reverted here and parked on ci/managed-workflows for the product migration after the Hub release.

The paired consumer is dokime.wt/remote-streamline, change release-v0-1-0. Harmos registry and native publication must finish before Dokime adopts version 0.1.1. No Hub or Dokime drop source is owned here.

## Inventory And Phase

Current phase is validating the release candidate. Owned files: Cargo.toml, nested package/fixture Cargo.toml and Cargo.lock files selected by release version policy, stkr.toml release/version fields, the tracked changelog and these OpenSpec lifecycle files. No runtime, binding or example API change is planned.

The generated preparation updated package carriers but left nine root workspace sibling requirements at 0.1.0; the candidate aligns those authored requirements to 0.1.1 with regenerated locks. The current rendered flow stays as is: Publish / Rehearsal runs on the release-branch push, Publish / Release runs on the stable tag with the approved release environment.

Provider state: dev requires the twenty rendered checks of the current graph plus signed commits; main allows merge commits, requires signed commits, forbids force pushes and deletion and accepts pushes only from the maintainer identity; stable tags are protected by the tag ruleset. The maintainer session holds the release authority of 2026-10-20: it pushes the signed tag and approves the release deployments with a ping.

Review README.md and existing usage against the versioned candidate. No new rustdoc or docs work. PR16's canonical license correction is separately reviewed and integrated after release.

After verified publication, add the README header badges for dev CI, crates.io harmos, latest GitHub release and Apache-2.0, preferably with the license/README follow-up. No PyPI badge. These badges do not gate v0.1.1 publication.

## Acceptance And Authority

Run env -u TMPDIR mise run check before every push. The candidate run on release/v0.1.1 must pass every configured Linux and Windows job including Package and Publish / Rehearsal. The release review targets main with the repository template and merges with a merge commit so ancestry is preserved; the merged tree must equal the rehearsed candidate tree. The signed tag v0.1.1 points at the merged main commit; its run publishes the crates in dependency order and the GitHub release. Then main is incorporated into dev with a merge-commit review. Never change v0.1.0. Preserve its failed draft until v0.1.1 is verified.

Benchmarks are excluded. Worktree retirement is separate; no automatic deletion without fresh eligible integration evidence.
