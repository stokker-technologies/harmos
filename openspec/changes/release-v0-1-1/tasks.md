# Tasks

## 1. Source

- [x] 1.1 Adopt PR19's dependency corrections; verify merged provider state and unchanged runtime APIs.
- [x] 1.2 Generate aligned v0.1.1 manifests, sibling requirements and locks; inspect the complete candidate diff.
- [x] 1.3 Keep the candidate on the existing rendered flow; park the managed-workflow adoption on ci/managed-workflows.
- [ ] 1.4 Review README claims and examples against candidate dependencies and final downloads; update only affected content.

## 2. Validation And Publication

- [ ] 2.1 Pass relevant local gates before signed publication; verify remote candidate identity.
- [ ] 2.2 Pass the complete native Linux/Windows graph, Package and Publish / Rehearsal on the candidate push.
- [ ] 2.3 Open the release review into main with the template, merge with a merge commit, verify the merged tree equals the candidate tree.
- [ ] 2.4 Push the signed tag v0.1.1, approve the release deployments, verify crates.io, release assets, checksums and native install jobs.
- [ ] 2.5 Incorporate main into dev with a merge-commit review; integrate PR16 and verify license changes.
- [ ] 2.6 Remove only the failed v0.1.0 draft after verified v0.1.1 publication; preserve its burned tag.
- [ ] 2.7 After release, add and verify README header badges for dev CI, crates.io harmos, latest release and Apache-2.0 with the license/README follow-up.

## Workflow Follow-Up

- Finish the change after all tasks pass; retain the workspace until publication and dev incorporation are verified.
