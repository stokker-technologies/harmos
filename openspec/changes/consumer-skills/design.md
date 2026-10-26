# Consumer Skill Design

## Agreed Boundaries

All product crates, including interfaces, live under crates/. The entry library exports the domain by default and the runtime through an optional runtime feature. Its prelude.rs exposes curated imports. The runtime composes execution resources; domain is independent, not beneath runtime.

Reusable operations are declared in product-domain and implemented in product-runtime. A self-contained crate colocates declaration and handler in vertical operations files. Plugin-private operations use that layout; reusable product contracts never move into the interface. External automation and gRPC clients can remain cohesive resource libraries. An existing application execution owner maps to the runtime role.

Portable settings and validation belong in the domain; runtime owns configured resources. Interface configuration and Dokime-specific settings retain their own roles. No universal boot.rs or empty module tree is required. Crates owning fallible behavior use error.rs and thiserror; facades, thin binaries, procedural macros and foreign adapters need no artificial error wrapper.

Native plugins can compose a reusable product runtime through its facade, sharing resources and owning startup, failure cleanup and shutdown. Domain declaration catalogs advertise contracts; handler catalogs bind implementations. Carrier context and Wasm dependencies constrain reuse. Dokime-specific hooks, suites and evidence stay outside these skills.

## File Inventory

Architecture: .agents/skills/harmos-architecture/SKILL.md and references/workspace.md.
Authoring: .agents/skills/harmos-authoring/SKILL.md and references/{operations,runtime,storage,plugins,transport}.md.
Testing: .agents/skills/harmos-testing/SKILL.md and references/runtime-and-carriers.md.
Documentation: README.md for discovery; CONTRIBUTING.md for retained repository-only testing ownership.
Generated copies: skill/provider outputs and current Hub conventions from stkr project update --no-provider. Retain root .github/workflows outputs, tools/python/src/harmos_tools/{build/cargo,ci/simulate,release/run}.py, and corresponding generated guest/sidecar starter workflows/tooling under crates/cargo-harmos/templates/. Preserve generated ownership and provider protections.
Specification: this change, folded into openspec/specs/consumer-guidance/spec.md on completion.

## Validation

Review realistic consumer layouts and current source API signatures qualitatively. Validate skill frontmatter and local links, reject unfinished scaffolds and external company-skill dependencies. Do not test authored prose or add compiled fixtures for these guidance edits. Review and syntax-check generated tooling/workflow changes with targeted existing checks. Run strict OpenSpec validation and inspect CI impact. Existing CI rules remain unchanged; convention updates and OpenSpec paths select full proof. After authorized push, start the default all-platform ci:simulate alongside hosted CI and enable protected review auto-merge without monitoring integration. Preserve actual local failures or pending proof.
