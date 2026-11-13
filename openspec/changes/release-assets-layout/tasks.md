# Release Assets Layout Tasks

## 1. Implementation

- [x] 1.1 Attach the protocol, conformance and compliance archives, `release.json` and `SHA256SUMS`; move the changelog into the release body.
- [x] 1.2 Make Nightly publish the same layout from the same tasks as a stable release.
- [x] 1.3 Attach `harmos-reports-<version>.zip` to Nightly and add the results table to its notes.
- [x] 1.4 Update the rehearsal, the README and the product guide.
- [x] 1.5 Attach `benchmarks.json` to Nightly under its fixed name.
- [x] 1.6 Extract the shared validation, publish and verify jobs as reusable workflows and make Nightly call them with the nightly channel.
- [x] 1.7 Align the Nightly contract with Dokime: triggers, cancellation, freshness, names, artifact interface, publish entry point, verification and report; stop cancelling Nightly runs.
- [x] 1.8 Align shared review input names and report calls with Dokime; collect every `results-*` artifact and replace the inventory through the shared draft publisher.
- [x] 1.9 Download and hash every draft/public asset regardless of API digests; let the oldest duplicate revision proceed; cover both with tooling stories.
- [x] 1.10 Document the remaining product differences and Dokime follow-ups; review README impact.

## 2. Final Validation

- [ ] 2.1 Run the release rehearsal and one Nightly, and compare the published asset list with the layout.
- [x] 2.2 Run format, repository and Rust checks, Python tooling lint/type checks, touched tooling stories and the requested Tests archive build.
- [ ] 2.3 Run the complete Python check with matching Product-generated clients and the reference host available.

## Workflow Follow-Up

- Dokime carries the same layout in its change `release-layout`.

- Preserve 1.7 freshness, noncancelling runs, publication identity and digest checks.
- Shared review inputs use `build-run`, `build-sha` and `build-version`; validation
  keeps Harmos's single `selection` dispatcher. All suite evidence uses `results-*`.
- Dokime must select `build-run || github.run_id` for its Windows Rust packages in
  shared publish/verify; report reconstruction here reuses hash-verified public
  downloads rather than requiring package restaging. Keep the four-hour schedule.
- Hosted rehearsal and Nightly publication remain owner proof in 2.1; no push is authorized.
- The index follows the shared `notes`, `pub_date`, `downloads` shape without desktop
  updater or plugin sections. `downloads.artifacts` excludes the index and checksums
  to avoid circular hashes. Conformance names use declared Rust triples.
- Compliance inventories conservatively cover the locked Cargo target graph (including
  build/development dependencies) and Python SDK lock, with hash-verified wheel license
  text. Inventories and notices travel inside Package artifacts, never as attachments.
- Reusable validation selects a job and channel; callers retain dependency edges so
  Nightly publication waits only for Product and Package. Registry publication stays
  in the protected Release workflow. Run-owned report updates replace a complete draft
  inventory, including reports, index and checksums, under the publication concurrency group.
- Native release dry-run/rehearsal still requires `target/package/<profile>/<target>/`;
  the complete Python check also needs Product-generated Python clients.
- Provider follow-ups from the workflow audit remain owner work: main's required
  checks, unused full-validation/mirror environments and mirror credential scope.
- Draft-review routing, release completion predicates and compiler-cache saver
  consolidation remain separate workflow work.
