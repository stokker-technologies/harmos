- [x] 0.1 Use one entry per release line showing its latest patch; `latest` points at the newest stable line and `dev` at the development branch.
- [x] 0.2 Use Starlight for the guides, search and selector; remove the superseded guide trial.
- [x] 1.1 Add the site project over `docs/` with static search and the release selector.
- [x] 1.2 Add `docs:build` and `docs:serve`.
- [x] 2.1 Generate the operations reference from the final [operation-boundary](../operation-boundary/tasks.md) contract, distinguishing native-only host operations, product wrappers and plugin protocol facilities; do not publish the retired host-client projection as callable API.
- [ ] 2.2 Verify pinned Rust/Python/C#/C++ reference generators against the final SDK source and stubs, then generate all four references after plugin-lifecycle harmonizes verbs/hooks/catalog and operation-boundary/plugin-boundary remove host-client and product-authoring ownership. Preserve static stub analysis for native Python modules.
- [ ] 2.3 Close doc comment gaps the generators report in each SDK.
- [x] 3.1 Build the documentation archive in the release pipeline as a required asset in the index; phase one contains guides and the operations reference until 2.2 adds the SDK references.
- [x] 3.2 Build the documentation archive in Nightly as a parallel job that may fail and report it.
- [x] 3.3 Add the pull request documentation build check.
- [x] 4.1 Add `docs:image`: assemble published archives, write the selector entries and `latest`.
- [ ] 4.2 Deploy the image.
- [ ] 5.1 Hosted pipelines publish the archive and the assembled site shows `latest`, a release and `dev`.

## Site Alignment

- [x] 6.2 Port the reference visual language, typography, logos, controls and 56rem content column into one local stylesheet.
