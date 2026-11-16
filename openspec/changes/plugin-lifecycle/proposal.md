# Plugin Lifecycle

## Why

Lifecycle verbs differ by placement and language: the runtime launches and shuts down, native
plugins have only a context-free `on_stop`, Wasm guests have `on_initialize`, `on_shutdown`
and `on_cleanup`, and SDKs serve. Declarations are composed by builder chains whose Rust type
grows with each typed record, and plugins acquire resources while declaring. The owner locked
one lifecycle vocabulary and one registration shape on 2026-11-16 so that every placement and
every language reads the same way.

## What Changes

Owner decisions locked on 2026-11-16. Status per row: **proposed** until its tasks land.
Harmos owns the generic plugin lifecycle and registration machinery; products own their kinds.

| Decision | Direction |
| --- | --- |
| Paired verbs | Verbs come only in pairs and each placement uses only the pairs it needs: `initialize` ↔ `finalize` acquire and release resources, and finalize always runs once initialize was attempted, including after a failed initialize; `start` ↔ `stop` begin and end activity. `register` is the declaration and admission phase and has no counterpart. |
| Host verbs | **BREAKING.** A product-built runtime goes new → register → start → stop; `run` is start, wait, stop. `launch` becomes `start`, `shutdown` becomes `stop` and `serve` becomes `run` wherever they denote these. |
| Plugin phases | **BREAKING.** A plugin goes register (admission) → initialize → finalize. Native `on_stop` and Wasm `on_shutdown`/`on_cleanup` merge into finalize: the host cancels in-flight work, then calls finalize. Plugins have no start or stop. |
| Registration is admission | The plugin declares streams, operations, settings, requirements and product sections; the host only checks definitions (identities, schemas, references, permissions, limits). Registration runs no handler, prompts nothing and acquires nothing. It is local declaration composition completed before serving, reported through the existing `Instance.Get` declarations and frozen per activation; no new host-invoked registration phase. |
| Registration API | **BREAKING.** `fn register(ctx: &mut Registrar) -> Result<()>` with `ctx.register(x::catalog())`; typed `register<T: Registration>` accumulates descriptors internally, removing builder type growth. Every language uses `register`, except C++, where it is a reserved keyword and the call is `add` (the phase is still named register). |
| Module entry points | Every module that contributes declarations exposes `catalog()`; `declarations()` stays the frozen host-facing description. |
| One vocabulary | Verbs, names and structure are the same across Rust, Python, C# and C++ with idiomatic casing only, and match the product SDKs built on them. |
| No aliases or path overrides | Code refers to types and modules by their qualified path. No import aliases (Rust `use x as y`, Python `import x as y`, C# `using X = Y;`, C++ renaming aliases) and no path overrides (`#[path]`, `sys.path` edits), except named unavoidable exceptions. |
| SDK ownership | The four plugin SDKs stay in Harmos; product kinds stay in product SDKs and compose through Harmos declaration sections. |

- Add host-invoked plugin initialize and finalize to the native plugin carrier, replacing
  `Instance.Stop`; rename the Wasm guest shutdown command and policy to finalize.
- Add the registrar, the `Registration` trait and its equivalents in all four SDKs; dissolve
  the Wasm `Registration` builder into the plugin value plus guest capacity.
- Rename application, service and carrier `cleanup` hooks to `finalize`.
- Security checks and prerequisite UI after admission and before initialize are a possible
  later phase, not part of this change.

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

`proto/harmos/plugin/v1` (`instance.proto`, `guest.proto`), `proto/harmos/model/v1`
(declaration lifecycle policies), `crates/harmos-runtime` (runtime verbs, plugin activation,
Wasm guest host, carrier), `crates/harmos-operations` (application, service and carrier
hooks), all four SDKs and their generators, `tools/conformance`, the Schematic example,
runtime stories, `docs/architecture.md`, `docs/product-guide.md`, `docs/plugin-manager.md`
and the four SDK guides. Products adopt the landed revision in their own change.
