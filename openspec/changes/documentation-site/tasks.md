# Tasks

Proposed on 2026-11-13. Implementation waits for the owner and follows the SDK restructures and
the examples.

## 0. Decisions Before Implementation

- [x] 0.1 Decided by the owner on 2026-11-14: an entry is a release line showing its latest patch; `latest` points at the newest stable line and `dev` at the development branch.
- [ ] 0.2 Resolve the generator decision: the supplied triage calls Starlight selected, but the proposal still records Starlight as proposed and a guide trial as the deciding step. Confirm the owner decision before replacing that trial; no site implementation in this tree establishes a selection.
- [ ] 0.3 Verify the reference generator for each language is current and handles the SDK as built.

## 1. Site and Guides

- [ ] 1.1 Add the site project over `docs/` with static search and the release selector.
- [ ] 1.2 Add `docs:build` and `docs:serve`.

## 2. References

- [ ] 2.1 Generate the operations reference from the final [operation-boundary](../operation-boundary/tasks.md) contract, distinguishing native-only host operations, product wrappers and plugin protocol facilities; do not publish the retired host-client projection as callable API.
- [ ] 2.2 Generate the Rust, Python, C# and C++ SDK references after [plugin-lifecycle](../plugin-lifecycle/tasks.md) harmonizes register/initialize/finalize, host start/stop/run and catalog(), and [operation-boundary](../operation-boundary/tasks.md) removes host-client resources.
- [ ] 2.3 Close doc comment gaps the generators report in each SDK.

## 3. Publication

- [ ] 3.1 Build the documentation archive in the release pipeline as a required asset in the index.
- [ ] 3.2 Build the documentation archive in Nightly as a parallel job that may fail and report it.
- [ ] 3.3 Add the pull request documentation build check.

## 4. Site Image

- [ ] 4.1 Add `docs:image`: assemble published archives, write the selector entries and `latest`.
- [ ] 4.2 Deploy the image.

## 5. Final Validation

- [ ] 5.1 Hosted pipelines publish the archive and the assembled site shows `latest`, a release and `dev`.
