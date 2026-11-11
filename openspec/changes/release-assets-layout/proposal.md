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
```

- Crates and the language SDK packages are published to their registries, not attached.
- The changelog is the release body, not an asset.
- Nightly attaches the reports archive and summarizes coverage, benchmarks beyond their threshold
  and suite results in its notes. A failed benchmark or coverage job does not prevent publication.

## Impact

Release and Nightly tooling and workflows, the rehearsal, the README and product guide.
