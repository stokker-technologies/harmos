<!-- Rendered by stkr project update from .agents/GUIDANCE.md; sha256:7cb387f9f1d03be8bb14c46be7db6b1b85d496e233877da8f89073bdcc8ea0c5 -->

# Project Guidance

## Configuration Policy

- Consumers explicitly supply resource and operational policy, including storage budgets, admission limits, concurrency and response batch sizes. Harmos validates configuration rather than silently selecting product defaults. Do not provide implicit Default implementations or deserialization fallbacks for these consumer-owned policies. Consumers own machine-aware sizing and operator overrides. Protocol constants, correctness constraints and private algorithm choices remain Harmos-owned; do not expose incidental implementation details merely to eliminate constants. This is the authoring direction; migrate existing APIs and consumers cohesively rather than claiming current defaults are already removed.

- Each configurable subsystem owns settings and validation in its local `config.rs`; root `config.rs` composes those types. Keep at most two levels within a subsystem, excluding root composition. Storage configuration types use the `Storage` prefix.
