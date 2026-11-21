# Project Guidance

## Configuration Policy

- Consumers own storage budgets, admission limits, concurrency and response batch sizes; Harmos owns protocol constants and correctness constraints.

## Documentation

- Start at `README.md` and `docs/index.md`; its Documentation section links the topic guides in `docs/`.
- Read `docs/project/architecture.md` before proposing architecture, protocol or public API changes. The owner writes the documentation; report documentation impact instead of editing `docs/`.
- Keep documentation in `docs/` and refer to code by path. Do not add `AGENTS.md`, `CLAUDE.md` or other Markdown notes inside code directories.

## Validation

- Run targeted tasks locally. Hosted CI and Nightly provide full validation.
