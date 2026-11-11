# Release Assets Layout Tasks

## 1. Implementation

- [ ] 1.1 Attach the protocol, conformance and compliance archives, `release.json` and `SHA256SUMS`; move the changelog into the release body.
- [ ] 1.2 Make Nightly publish the same layout from the same tasks as a stable release.
- [ ] 1.3 Attach `harmos-reports-<version>.zip` to Nightly and add the results table to its notes.
- [ ] 1.4 Update the rehearsal, the README and the product guide.
- [ ] 1.5 Attach `benchmarks.json` to Nightly under its fixed name.
- [ ] 1.6 Extract the shared validation, publish and verify jobs as reusable workflows and make Nightly call them with the nightly channel.
- [ ] 1.7 Align the Nightly contract with Dokime: triggers, cancellation, freshness, names, artifact interface, publish entry point, verification and report; stop cancelling Nightly runs.

## 2. Final Validation

- [ ] 2.1 Run the release rehearsal and one Nightly, and compare the published asset list with the layout.

## Workflow Follow-Up

- Dokime carries the same layout in its change `release-layout`.
