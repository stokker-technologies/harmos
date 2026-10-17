<!-- Generated from .agents/GUIDANCE.md; edit .agents/GUIDANCE.md and run `stkr project update` -->

# Project Guidance

## Configuration Policy

- Consumers explicitly supply resource and operational policy, including storage budgets, admission limits, concurrency and response batch sizes. Harmos validates configuration rather than silently selecting product defaults. Do not provide implicit Default implementations or deserialization fallbacks for these consumer-owned policies. Consumers own machine-aware sizing and operator overrides. Protocol constants, correctness constraints and private algorithm choices remain Harmos-owned; do not expose incidental implementation details merely to eliminate constants. This is the authoring direction; migrate existing APIs and consumers cohesively rather than claiming current defaults are already removed.

- Each configurable subsystem owns settings and validation in its local `config.rs`; root `config.rs` composes those types. Keep at most two levels within a subsystem, excluding root composition. Storage configuration types use the `Storage` prefix.
