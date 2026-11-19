# Plugin Lifecycle

## Why

Lifecycle verbs differ by placement and language: the runtime launches and shuts down, native
plugins have only a context-free `on_stop`, Wasm guests have `on_initialize`, `on_shutdown`
and `on_cleanup`, and SDKs serve. Declarations are composed by builder chains whose Rust type
grows with each typed record, and plugins acquire resources while declaring. The owner locked
one lifecycle vocabulary and one registration shape on 2026-11-16 so that every placement and
every language reads the same way.

## What Changes

Owner decisions locked on 2026-11-16, with native lifecycle operations superseded on 2026-11-19. Status per row: **proposed** until its tasks land.
Harmos owns the generic plugin lifecycle and registration machinery; products own their kinds.

| Decision | Direction |
| --- | --- |
| Paired verbs | Verbs come only in pairs and each placement uses only the pairs it needs: `initialize` ↔ `finalize` acquire and release resources, and finalize always runs once initialize was attempted, including after a failed initialize; `start` ↔ `stop` begin and end activity. `register` is the declaration and admission phase and has no counterpart. |
| Host verbs | **BREAKING.** A product-built runtime goes new → register → start → stop; `run` is start, wait, stop. `launch` becomes `start`, `shutdown` becomes `stop` and `serve` becomes `run` wherever they denote these. |
| Plugin phases | **BREAKING.** Local register → host admission → initialize → finalize. Native `on_stop` and Wasm `on_shutdown`/`on_cleanup` merge into finalize: the host cancels in-flight work, then calls finalize. Plugins have no start or stop. |
| Registration is admission | The plugin declares streams, operations, settings, requirements and product sections; the host only checks definitions (identities, schemas, references, permissions, limits). Registration runs no handler, prompts nothing and acquires nothing. It is local declaration composition completed before serving, reported through the existing `Instance.Get` declarations and frozen per activation; no new host-invoked registration phase. |
| Registration API | **BREAKING.** `fn register(ctx: &mut Registrar) -> Result<()>` with `ctx.register(x::catalog())`; typed `register<T: Registration>` accumulates descriptors internally, removing builder type growth. Every language uses `register`, except C++, where it is a reserved keyword and the call is `add` (the phase is still named register). |
| Module entry points | Every module that contributes declarations exposes `catalog()`; `declarations()` stays the frozen host-facing description. |
| One vocabulary | Verbs, names and structure are the same across Rust, Python, C# and C++ with idiomatic casing only, and match the product SDKs built on them. |
| No aliases or path overrides | Import the names used; qualify clashes by module path. No import aliases or path overrides (`#[path]`, `sys.path` edits), except named unavoidable exceptions. |
| SDK ownership (later owner amendment: plugin-boundary) | Harmos owns thin four-language carrier helpers and generic registration/lifecycle machinery. Products own contracts, registrars and the only author-facing SDKs/starters; Harmos types stay internal without import aliases. |

- Encode host-invoked native initialize and finalize as reserved `<plugin>.initialize` and
  `<plugin>.finalize` operations through existing Invoke; preserve Instance.Stop and all wire messages.
- Native hook policies use existing operation policies/defaults, without new configuration.
- Plugin-lifetime child subscriptions cannot outlive Invoke/window contexts on the current wire; leave them out rather than emulate them.
- Add generic registration accumulation in all four carrier helpers; products own
  their public registrar and `Registration` contract and translate internally. Dissolve
  the Wasm `Registration` builder into the plugin value plus guest capacity.
- Rename application, service and carrier `cleanup` hooks to `finalize`.
- Security checks and prerequisite UI after admission and before initialize are a possible
  later phase, not part of this change.

`plugin-boundary` keeps the internal manager and register → admission → initialize
→ finalize state machine in Harmos; installation, registry, persisted settings,
admission policy and management operations move to products. Registration is local
and inert; host admission combines generic checks with the product kind's callback.
Regenerate the carrier, facade and lifecycle together once with `operation-boundary`
and `plugin-boundary`, preserving every phase guarantee below.

Supersedes, in the `grpc-plugins` change: the Wasm registration sketch (`on_initialize`,
`on_shutdown`), the "Plugin initialization and shutdown operations" row, "Before the host
shuts down or removes a plugin it calls `Stop` on it. The plugin runs its stop and cleanup
hooks" and the Authoring sketch's builder chain (`.operation(..)`, `.stream(..)`,
`.observer(..)`) and `plugin.serve()`, together with the `plugin-protocol` delta scenario
where each plugin runs its stop and cleanup hooks.

## Capabilities

### New Capabilities

- `host-lifecycle`: the verbs of a product-built runtime and of SDK serving entry points.
- `plugin-lifecycle-hooks`: plugin admission, host-invoked initialize and finalize, and
  finalize guarantees across native and Wasm placements.
- `plugin-registration`: the registrar, typed registrations, module catalogs, product
  sections and admission checks in all four SDKs.

### Modified Capabilities

- `resource-store`: setup errors are reported by `start`, and setup failure releases prepared
  resources through finalize.
- `runtime-registration`: service restart waits for finalize, and explicit stop does not
  restart services.

## Impact

No native protocol changes. `crates/harmos-runtime` (runtime verbs, plugin activation,
Wasm guest host, carrier), `crates/harmos-operations` (application, service and carrier
hooks), all four SDKs and their generators, `tools/conformance`, the Schematic example,
runtime stories, `docs/architecture.md`, `docs/product-guide.md`, `docs/plugin-manager.md`
and the four SDK guides. Products adopt the landed revision in their own change.
