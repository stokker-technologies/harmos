# Tasks

## Implementation

- [x] 1.1 Port and harden the owned production modules against current dev.
- [x] 1.2 Expose the bounded spool through the existing Harmos facade and define its Dokime adoption contract.
- [x] 1.3 Add deterministic regressions, review README impact and update the existing topic guide.
- [x] 1.4 Complete focused query and touched-module proof; review the full diff and required-term scan.

## Final Validation

- [x] 2.1 Run check:rust and report its result.
- [x] 2.2 Compare one existing query benchmark when the machine is quiet, otherwise report the skip.

## Workflow Follow-Up

- After the verified candidate push, run the default all-platform local CI simulation alongside hosted checks and report its actual results. Provider review, immediate squash, Nightly dispatch and explicitly requested worktree retirement follow the source lifecycle.
