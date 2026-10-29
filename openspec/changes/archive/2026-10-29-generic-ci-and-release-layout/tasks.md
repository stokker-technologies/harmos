# Tasks

## 1. Product Adoption

- [x] 1.1 Triage the provider-disabled render and retained exact sibling pins; existing source: `.github/workflows/release.yml:1`, `tools/python/src/harmos_tools/release/version.py:105`, `Cargo.toml:21`.
- [x] 1.2 Replace argument-bearing aggregate dependencies in `mise.toml` with native parallel task groups; native mise dry-runs preserve format, profile and benchmark flag expansion.
- [x] 1.3 Remove retired bundle release flags in `stkr.toml`, retain the declared Cargo tool archive, and update affected README/release guidance; Windows release staging selects only the tool ZIP, CHANGELOG.md and SHA256SUMS, excluding examples.
- [x] 1.4 Triage generated changelogs and credential-free rehearsal; existing owners: `tools/python/src/harmos_tools/package/changelog.py:49`, `tools/python/src/harmos_tools/release/run.py:454` and `:560`.

## 2. Targeted Validation

- [x] 2.1 Run local-platform release dry and inspect archive, release and crate changelog inventories; Windows release Dry passes for the tool ZIP and all ten library crates, with temporary source edits restored.
- [ ] 2.2 Prove credential-free local-platform rehearsal after the Hub resolves its unconditional all-target requirement in rendered `release/run.py`; preserve both product release targets.
- [x] 2.3 Validate OpenSpec strictly and check the owned diff; all 25 items pass after splitting the existing combined `native-ci` requirement without changing its deadline contract; other parked changes remain untouched.

## Workflow Follow-Up

- The caller integrates these commits and closes Hub adoption task 2.2 after resolving reported upstream defects and completing required native validation.
- The Hub applies credential removal to direct `crates-dry` dispatch (`release/run.py:498`), which currently bypasses the rehearsal wrapper.
- The owner registers crates.io Trusted Publishing against `release.yml`, environment `release`, before publication.
