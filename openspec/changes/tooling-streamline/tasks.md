# Tasks

## 1. Contract

- [x] 1.1 Fix the final entry-point arguments and workflow job list in proposal.md against the implemented dispatcher.
- [x] 1.2 Lock mise.toml, tools/python, workflow/action files and contributor guidance as the source inventory.

## 2. Tasks and Tooling

- [x] 2.1 Replace the task families with the entry points, with language and layer as arguments, and delete the tasks for removed facilities.
- [x] 2.2 Remove every `CARGO_INCREMENTAL` setting from `mise.toml` and tooling and set it once in the workflows.
- [x] 2.3 Remove storage preflight and its wrapper from this project-owned tooling copy; keep paired project interfaces harmonized.
- [x] 2.4 Prune the compatibility and extended selections to what still proves something.

- [x] 2.5 Align publication --dry/--stage and build --artifact fixtures with the paired interface.
- [x] 2.6 Accept --features on package-focused build/test commands and isolate Python SDK environments.
- [x] 2.7 Update the uv pin to 0.13.6 and declare protocol-generated source paths.

## 3. Workflows

- [x] 3.1 Move archiving, summaries, the Nightly freshness check and report, the release review and the mirror push into tasks.
- [x] 3.2 Rewrite the CI, Nightly and Release workflows as matrix job templates that each run one task.

## 4. Final Validation

- [ ] 4.1 Run every entry point locally on Windows and Linux, and confirm hosted CI and Nightly run the same tasks.
- [x] 4.2 Update the contributor guide.

- [ ] 4.3 Verify the installed repository checker accepts the entry-point task model and generated protocol declarations; hand remaining checker defects to its owner.

## Workflow Follow-Up

- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
