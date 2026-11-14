# Design

## Site Layout

```text
/latest/                     the newest stable release (same pages as its entry)
/<release>/                  guides of that release
/<release>/reference/operations/
/<release>/reference/rust/
/<release>/reference/python/
/<release>/reference/csharp/
/<release>/reference/cpp/
/dev/                        the same tree, built from the dev branch
/versions.json               the entries the selector offers, newest first
```

The selector offers `latest`, each supported release and `dev`. `latest` resolves to the newest
stable release; a prerelease or Nightly never becomes `latest`.

## Archives

| Channel | Asset | Source | Site Entry |
| --- | --- | --- | --- |
| Stable release | `harmos-docs-<version>.zip` | the release tag | `/<release>/`, and `/latest/` when newest |
| Nightly | `harmos-docs-nightly.zip` | the `dev` branch | `/dev/` |

An archive holds the built guides and the five references for exactly its source, generated with
the tools that source pins. The release index lists it like every other asset.

## Assembly

`docs:image` downloads the archives of the supported releases and the Nightly archive, unpacks each
under its entry, writes `versions.json` and `/latest/`, and packs the image. It installs no language
toolchain and builds no old release. Only releases that published an archive can appear.

## References

Each generator writes its own static pages under `reference/<name>/`; the guides link to them.
They are not converted into site pages.

| Reference | Source | Generator (to Verify) |
| --- | --- | --- |
| Operations and messages | protocol files and their comments | a protocol documentation generator |
| Rust SDK | doc comments | Rustdoc |
| Python SDK | docstrings and type stubs | a stub-aware generator |
| C# SDK | XML documentation comments | DocFX |
| C++ SDK | header comments | Doxygen |

A native Python module is documented from its stubs, so a generator that only imports the module
does not qualify.

## Tasks

```text
docs:build    guides and references for the current checkout, into target/docs/<entry>/
docs:serve    local preview of the current checkout
docs:image    assemble the published archives into the site image
```

## Pipelines

- Pull requests run a documentation build check.
- Nightly builds the archive in a parallel job that may fail; publication depends only on build and
  package, and the final report says whether the archive was attached.
- A stable release requires the archive.

## Search

Search is static and needs no external service, so the site works on a private deployment.
