# Release Assets Layout Tasks

## 1. Implementation

- [x] 1.1 Attach the protocol, conformance and compliance archives, `release.json` and `SHA256SUMS`; move the changelog into the release body.
- [x] 1.2 Make Nightly publish the same layout from the same tasks as a stable release.
- [x] 1.3 Attach `harmos-reports-<version>.zip` to Nightly and add the results table to its notes.
- [x] 1.4 Update the rehearsal, the README and the product guide.
- [x] 1.5 Attach `benchmarks.json` to Nightly under its fixed name.
- [x] 1.6 Extract the shared validation, publish and verify jobs as reusable workflows and make Nightly call them with the nightly channel.
- [x] 1.7 Align the Nightly contract with Dokime: triggers, cancellation, freshness, names, artifact interface, publish entry point, verification and report; stop cancelling Nightly runs.

## 2. Final Validation

- [ ] 2.1 Run the release rehearsal and one Nightly, and compare the published asset list with the layout.

## Workflow Follow-Up

- Dokime carries the same layout in its change `release-layout`.

- Source inventory, in dependency order: release layout/staging and package compliance
  (`tools/python/src/harmos_tools/{release,package}/`, `tools/project.toml`); shared
  channel dispatch (`ci/tasks.py`, `mise.toml`); Nightly reports and benchmark export
  (`release/reports.py`, `ci/workflow.py`, `test/run.py`); release rehearsal and
  affected README/product/tooling guides; reusable validation/publication/verification
  workflows (`.github/workflows/`); focused tooling stories under `tools/python/tests/`.
- Preserve 1.7 freshness, noncancelling runs, publication identity and digest checks.
- Hosted rehearsal and Nightly publication remain owner proof in 2.1; no push is authorized.
- The index follows the shared `notes`, `pub_date`, `downloads` shape without desktop
  updater or plugin sections. `downloads.artifacts` excludes the index and checksums
  to avoid circular hashes. Conformance names use declared Rust triples.
- Compliance inventories conservatively cover the locked Cargo target graph (including
  build/development dependencies) and Python SDK lock, with hash-verified wheel license
  text. Inventories and notices travel inside Package artifacts, never as attachments.
- Reusable validation selects a job and channel; callers retain dependency edges so
  Nightly publication waits only for Product and Package. Registry publication stays
  in the protected Release workflow. Run-owned report updates replace report downloads,
  index and checksums under the same publication concurrency group.
- Clean-tree tooling stories prove both channel inventories, manifest fields, archive
  members, ABI refusal, wrong directory shapes, locked license hashes, report reruns,
  stale publication refusal and benchmark export after runner failure.
- Native release dry-run/rehearsal still requires `target/package/<profile>/<target>/`;
  this worktree has no Product/package outputs. The complete Python check also needs
  Product-generated Python clients. These remain real-input proof, not source closure.
- Provider follow-ups from the workflow audit remain owner work: main's required
  checks, unused full-validation/mirror environments and mirror credential scope.
- Draft-review routing, release completion predicates and compiler-cache saver
  consolidation remain separate workflow work.
