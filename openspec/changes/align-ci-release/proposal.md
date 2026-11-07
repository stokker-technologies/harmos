# Align CI and Release Structure

## Why

Harmos needs the same project-owned CI and release boundaries as Dokime's
merged PR #199. Dev builds supply fresh downloads; release publication requires
green evidence for the exact release commit.

## What Changes

- Keep hosted runners, caches, PR suites and the complete nightly workload.
- Run only quick checks and Product and Tests compilation on dev pushes; dev merges stay ungated.
- Allow git dependencies in development; reject them on release reviews and publication.
- Verify the release commit's required checks before tagging or publishing.
- Replace one mutable nightly release while drafted, verify all downloads,
  move its tag once, then publish the complete set.
- Document project CI ownership and the owner-managed main ruleset.

## Impact

This branch owns workflow/action changes, Python CI/release tooling, focused
tooling tests, task declarations, contribution guidance and affected CI specs.
Package catalogs supply tool archives, Schematic ZIPs and plugin bundles.
PR #128's platform-split optimization is excluded: Dokime retains matrix jobs,
and changing scheduling topology would exceed this alignment's structure.
Product code, product tests and provider settings are excluded. Delivery is one
ready PR into dev, with its CI reported once and no merge or auto-merge.
