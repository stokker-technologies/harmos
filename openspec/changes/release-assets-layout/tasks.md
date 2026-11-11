# Release Assets Layout Tasks

## 1. Implementation

- [ ] 1.1 Attach the protocol, conformance and compliance archives, `release.json` and `SHA256SUMS`; move the changelog into the release body.
- [ ] 1.2 Make Nightly publish the same layout from the same tasks as a stable release.
- [ ] 1.3 Attach `harmos-reports-<version>.zip` to Nightly and add the results table to its notes.
- [ ] 1.4 Update the rehearsal, the README and the product guide.
- [ ] 1.5 Attach `benchmarks.json` to Nightly under its fixed name.
- [ ] 1.6 Extract the shared validation, publish and verify jobs as reusable workflows and make Nightly call them with the nightly channel.
- [x] 1.7 Align the Nightly contract with Dokime: triggers, cancellation, freshness, names, artifact interface, publish entry point, verification and report; stop cancelling Nightly runs.

## 2. Final Validation

- [ ] 2.1 Run the release rehearsal and one Nightly, and compare the published asset list with the layout.

## Workflow Follow-Up

- Dokime carries the same layout in its change `release-layout`.

- 1.7 preserves Harmos's native artifact interface and the existing staged publisher's
  tag, release identity and asset verification. Nightly uses UTC schedule/manual
  triggers, noncancelling runs, fail-closed freshness and idempotent run-owned reports.
- Shared workflow extraction remains 1.6. Release assets, reports, benchmark uploads
  and a real publication rehearsal remain open in 1.1–1.5 and 2.1.
- Provider follow-ups from the workflow audit remain owner work: main's required
  checks, unused full-validation/mirror environments and mirror credential scope.
- Draft-review routing, release completion predicates and compiler-cache saver
  consolidation remain separate workflow work; this task does not introduce reusable workflows.
