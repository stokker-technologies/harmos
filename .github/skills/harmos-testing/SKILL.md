---
name: harmos-testing
description: Test products using Harmos at domain, runtime, resource and carrier boundaries, including Harmos's shipped consumer examples.
---
<!-- Rendered by stkr project update from .agents/skills/harmos-testing/SKILL.md; sha256:b235c4ec48f5057a77b61064d852e3d0aa386c2e8139e98c1a64047672d931a0 -->

# Harmos Consumer Testing

Test the behavior the product owns through the boundary its consumers use.
This guidance applies to external products and Harmos's shipped example consumers.
Use the consuming project's test commands and validation policy; no Harmos
repository fixture pipeline or company tooling is required.

## Select The Owner

All product crates, including interfaces, live under `crates/`. For a reusable
product, test portable contracts and configuration validation in `product-domain`
and the operations crate without `runtime`; test execution through the operations
crate with `runtime` and configured composition in `product-runtime`. Keep
domain tests independent of runtime initialization. The entry library exposes
the domain by default and can expose runtime through an optional `runtime`
feature; check the feature combinations the product promises.

A self-contained operation keeps its declaration and handler together in a
vertical operation module, including plugin-private operations. Test it there;
do not introduce a separate domain crate solely for tests. Reusable operation contracts
keep their operations owner when a CLI, plugin or other interface consumes them.

Resource libraries such as automation or gRPC clients retain their own protocol
and failure tests. Runtime composition tests prove how the product uses those
resources; they need not turn the resource library into a product interface.

## Choose The Smallest Useful Boundary

Test pure validation and transformations directly. Exercise dispatch when
registration, caller context, streaming, cancellation or resource ownership is
part of the behavior. Add a real carrier or process only when its boundary can
change the outcome. Assert observable results, durable state and completed
cleanup rather than private scheduling details or elapsed-time guesses.

Read [Runtime And Carriers](references/runtime-and-carriers.md) for composed
plugins, native/guest compatibility, lifecycle settlement and persistence tests.

Use deterministic synchronization to place failures before or after readiness.
Keep test-owned resources isolated and await their shutdown. Run focused checks
for changed behavior, then the consuming project's required validation; do not
copy Harmos's internal CI lanes, fixture names or packaging policy into a product.
