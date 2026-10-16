# Project Guidance

- Project-local guidance and skills are canonical under `.agents/`; regenerate provider copies with `stkr project update`. Do not author competing provider copies.
- Complete cohesive implementation before broad validation. Every project Python environment is the repository-root `.venv/`. Private task inputs follow the Hub task lifecycle.

- Development exception for Harmos: dev integration requires complete Linux checks, development-profile build, tests and exact-revision development-artifact proof. Windows runs independently with truthful failures and artifacts, without continue-on-error, and remains advisory for dev until the user explicitly restores its requirement. Keep the development gate/catalog independent of Windows completion and distinct from full release validation. Windows remains required for main and releases. Do not hardcode or automate restoration. This exception supersedes all-platform dev requirements only for Harmos dev.

## Configuration Policy

- Consumers explicitly supply resource and operational policy, including storage budgets, admission limits, concurrency and response batch sizes. Harmos validates configuration rather than silently selecting product defaults. Do not provide implicit Default implementations or deserialization fallbacks for these consumer-owned policies. Consumers own machine-aware sizing and operator overrides. Protocol constants, correctness constraints and private algorithm choices remain Harmos-owned; do not expose incidental implementation details merely to eliminate constants. This is the authoring direction; migrate existing APIs and consumers cohesively rather than claiming current defaults are already removed.

- Each configurable subsystem owns settings and validation in its local `config.rs`; root `config.rs` composes those types. Keep at most two levels within a subsystem, excluding root composition. Storage configuration types use the `Storage` prefix.
