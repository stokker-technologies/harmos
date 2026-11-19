# Plugin Lanes

Status: locked by the owner on 2026-11-19. Implementation is sequenced after
the lifecycle operations and the owned plugin listener, which change the same
admission and serving code.

## Why

Some plugin operations must never overlap, for example two commands to one
instrument, while others may run freely. The host already has lanes: named
admission groups with a serial, bounded or unbounded capacity, declared at
startup, with every operation registered into one. A plugin cannot use them:
all of a plugin's operations are admitted into one lane chosen by the host, and
every call site passes the default unbounded lane. Plugin authors therefore
build their own serialisation inside each plugin.

## What Changes

- A plugin declares its lanes, each with a name and a capacity (serial, a fixed
  concurrent width, or unbounded), and each of its operations names the lane it
  belongs to. Operations that name none use the plugin's default unbounded lane.
- At admission the host creates the declared lanes scoped to that plugin
  instance and admits every invocation of the plugin through its operation's
  lane, using the existing lane admission.
- The plugin description carries the two additive facts: the lane list and each
  operation's lane. **Additive** to the description; existing plugins are
  unchanged.
- Every SDK language exposes the declaration; no SDK enforces lanes itself.

## Capabilities

### New Capabilities

- `plugin-lanes`: declaration, validation, scoped creation and admission of
  plugin lanes.

## Impact

The plugin description model and its fingerprint inputs, plugin admission and
invocation dispatch in the runtime, the four SDK authoring layers, the
conformance stories. No change to the invoke call itself.
