# Documentation Site

## Why

The guides under `docs/` are plain Markdown read on the repository host. The Starlight site, operations reference, release selector, documentation archives and
image assembly are landed. Four language reference generators, comment coverage,
deployment and live publication proof remain open; references must match each release.

## What Changes

- One documentation website per product, served from one deployed image that contains the pages of
  every supported release line, a `latest` entry for the newest stable release and a `dev` entry
  built from the `dev` branch.
- Each stable release and each Nightly publishes one documentation archive: the built guides and
  the references for exactly that source.
- The site image is assembled from those archives; it never rebuilds an old release.
- References are generated for the operations (from the protocol files) and for the Rust, Python,
  C# and C++ SDKs.
- `docs:build`, `docs:serve` and `docs:image` tasks.

## Decisions

Publication scope was proposed on 2026-11-13. The owner settled release-line entries on
2026-11-14 and selected Starlight for phase one on 2026-11-17. SDK references follow the
SDK restructures; deployment remains open.

- Old releases are never copied into the repository. A release's documentation is the documentation
  at its tag. Maintained Markdown keeps stating current contracts only.
- One entry per supported release line showing its latest patch, plus `latest` and `dev`.
  `latest` selects the newest stable line; `dev` follows the development branch.
- Installers do not bundle the documentation. The website is the way documentation is read.
- Starlight renders guides and generated operations pages with static search; the guide trial is dropped.
- Phase one archives contain guides and the operations reference. Task 2.2 adds the four SDK references.
- The documentation archive is a required asset of a stable release. In Nightly it is built by a
  parallel job that may fail; Nightly publication does not depend on it.

## Release Readiness

The remaining tasks are release-blocking for the complete documentation deliverable:
four language references and comment coverage after the active boundary/lifecycle
cutover, deployment and live stable/dev/latest publication proof. Reference pages
cover Harmos's retained APIs and carrier helpers; product-owned author SDK references
belong to the product. Keep final publication proof unchecked.

## Open

- The reference generator per language, to be verified as current before adoption.
- The deployment target of the assembled image.

## Impact

- `docs/`, a new site project, the release and Nightly workflows, the release asset layout and its
  index, mise tasks.
- Doc comments become a deliverable in every SDK.
- Follows the SDK restructures and the examples, because the references are generated from them.
