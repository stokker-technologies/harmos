# Release Assets Layout

## Why

Dokime adopts one bundled asset layout for stable releases and Nightly. The two projects keep their
tooling duplicated but harmonized, so Harmos uses the same shape. Harmos's own release changes
anyway: `cargo-harmos`, today its only archives, is removed by `facility-trim`, and `grpc-plugins`
adds protocol definitions and language SDKs.

## What Changes

One layout for stable releases and for Nightly. Nightly is a release run without the tag and the
registry publication, plus benchmarks and coverage.

```text
harmos-proto-<version>.zip          harmos.v1 definitions and the behaviour specification
harmos-conformance-<version>-<platform>.zip|tar.gz   the reference host, one per platform
harmos-compliance-<version>.zip     every SBOM and the notices, in folders
release.json                        one index of the release's assets
SHA256SUMS
harmos-reports-<version>.zip        Nightly only: benchmark and coverage results
benchmarks.json                     Nightly only: latest benchmark results, fixed name
```

- Crates and the language SDK packages are published to their registries, not attached.
- The changelog is the release body, not an asset.
- Nightly attaches the reports archive and summarizes coverage, benchmarks beyond their threshold
  and suite results in its notes. A failed benchmark or coverage job does not prevent publication.

## Nightly and Release Share Jobs

Nightly is the release run with `channel=nightly`: no version tag, no registry publication, and
benchmarks and coverage added. The validation jobs of CI and the publish and verify jobs of
Release are reusable workflows that Nightly calls; it adds only freshness, benchmarks, coverage
and the report. Dokime and Harmos keep the same Nightly: triggers and timezone, a run that never
cancels, one freshness contract (tag comparison, duplicate revision detection, lookup errors fail
closed), stage and job names, one named artifact interface, one publish entry point, digest
verification, and an idempotent result report guarded by the publication identity. Products,
languages, matrices and asset inventories stay each project's own.

## Published Numbers

`benchmarks.json` is attached to every Nightly under that fixed name, so a site can read the
latest results from the Nightly release without expiry. It carries, per benchmark: name, suite,
unit, value, the baseline value and the revision. No history is kept in the repository.

## Impact

Release and Nightly tooling and workflows, the rehearsal, the README and product guide.
