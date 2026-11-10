# Tasks

## 1. Contract

- [ ] 1.1 Fix the final argument names of the entry points and the job list per workflow from `proposal.md`.
- [ ] 1.2 Lock the source inventory: `mise.toml`, `tools/python`, workflows, actions and the contributor guide.

## 2. Tasks and Tooling

- [x] 2.1 Replace the task families with the entry points, with language and layer as arguments, and delete the tasks for removed facilities.
- [x] 2.2 Remove every `CARGO_INCREMENTAL` setting from `mise.toml` and tooling and set it once in the workflows.
- [x] 2.3 Adopt the shared build tooling without the storage preflight and delete the local wrapper code that existed only for it.
- [x] 2.4 Prune the compatibility and extended selections to what still proves something.

## 3. Workflows

- [x] 3.1 Move archiving, summaries, the Nightly freshness check and report, the release review and the mirror push into tasks.
- [x] 3.2 Rewrite the CI, Nightly and Release workflows as matrix job templates that each run one task.

## 4. Final Validation

- [ ] 4.1 Run every entry point locally on Windows and Linux, and confirm hosted CI and Nightly run the same tasks.
- [x] 4.2 Update the contributor guide.

## Workflow Follow-Up

- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
