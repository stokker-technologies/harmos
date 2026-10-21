# Proposal

## Why

The partially published v0.1.0 cannot be reused. Publish the corrected workspace as v0.1.1 so Dokime can consume registry dependencies.

## What Changes

- Preserve PR19's path-only sibling development dependencies.
- Align package versions, sibling requirements and locks at 0.1.1.
- Release through the existing rendered flow: rehearsal on the release branch, merge-commit review into main, signed stable tag, approved publication, incorporation into dev.
- Keep the managed-workflow migration out of the candidate; it waits on the Hub release on branch ci/managed-workflows.

## Capabilities

### New Capabilities

### Modified Capabilities

No runtime contract changes; this change records release delivery work.

## Impact

Workspace and fixture manifests/locks, stkr.toml and the tracked changelog. README claims receive review; documentation expansion and HTTP/2 tuning are excluded.
