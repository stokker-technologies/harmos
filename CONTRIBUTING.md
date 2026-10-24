# Contributing

Help improve Harmos with focused fixes, clear API feedback and useful examples.
Discuss changes to public contracts before implementing them.

## Development Setup

Install [mise](https://mise.jdx.dev), then prepare the checkout:

```sh
mise trust
mise install --locked
mise run prepare:tools
```

Windows and Linux are supported. The [Schematic example](examples/schematic/README.md)
is a complete application for understanding composition, plugins and storage.

## Development Workflow

Preserve clear boundaries between domain contracts, runtime resources and
application-owned persistence. Update affected consumers and examples when
changing a contract; flag unrelated stale examples rather than hiding them.

Use Conventional Commit subjects and keep generated files with their source.
The root uv workspace owns the Python environment and lockfile.

## Tests And Checks

Run targeted checks while implementing. After all agreed source work is
complete, run the impact-selected local review gate:

```sh
mise run ci:simulate
```

The default run covers all declared required platforms. Prose-only changes use
lightweight checks; mixed and executable changes retain their affected proof.
Hosted CI remains required before integration.

Product and Tests compile independently with matching Clippy invocations.
Tests use the default unoptimized test profile; optional extended tests remain
explicit local work. Nightly runs ordinary verification with release Product
builds and downloadable unsigned packages, plus coverage and benchmarks.

## Pull Requests

Target `dev` and explain the user-visible problem, resulting behavior and
verification. Keep required checks enabled and include no private data or
credentials. Contributions use the [Apache-2.0 license](LICENSE).

Release preparation and publication follow the separate protected release
workflow; an ordinary pull request does not publish packages.

## Reporting Issues

Use the [issue tracker](https://github.com/stokker-technologies/harmos/issues).
Include the revision, operating system, minimal reproduction, expected result,
actual behavior and relevant sanitized logs.
