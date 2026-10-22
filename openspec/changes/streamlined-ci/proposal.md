# Streamlined CI Migration

## Why
Adopt the agreed Hub CI contract, remove obsolete simulation and blanket setup, and repair baseline failures so the product delivers through green native CI.

## What Changes
- Migrate authored tasks, project configuration, managed tooling/workflows and affected documentation together.
- Use language-second task names, Test stage, sparse job overrides, explicit dependencies, cache selection and impact policies where appropriate.
- Preserve required platform coverage, artifact ownership and release behavior; no weakened gates.

## Authorization
The user authorizes initial full baseline validation, migration, local proof, pushes, PR creation, CI monitoring/repair and merge to dev. No release publication. Baseline runs precede edits as an explicit proof-policy exception. Preserve unrelated work.

