# Proposal

## Why

The partially published v0.1.0 cannot be reused. Publish the corrected workspace as v0.1.1 so Dokime can consume registry dependencies.

## What Changes

- Preserve PR19's path-only sibling development dependencies.
- Align package versions, sibling requirements and locks at 0.1.1.
- Adopt the repaired, published Hub linear-release generator, replacing legacy workflows, tracked changelog and release manifests through their owner.
- Verify the exact release review on Linux and Windows, squash it into dev, then let Release / Tag tag that commit and fast-forward main before publication verification.

## Capabilities

### New Capabilities

### Modified Capabilities

No runtime contract changes; this change records release tooling and delivery work.

## Impact

Workspace and fixture manifests/locks, stkr.toml, legacy release notes/tooling/provenance, managed native workflows, provider protection/credential reconciliation and registry/assets publication. README claims receive review; documentation expansion and HTTP/2 tuning are excluded.
