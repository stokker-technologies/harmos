## Purpose

Define reusable plugin activation mechanics while products select plugins and own admission policy, installed state and author contracts.

## ADDED Requirements

### Requirement: Product Selects Activation

Harmos SHALL load only the plugin kind and placement explicitly selected by a product. Products SHALL own installed-plugin discovery, registry, installation, bundles, artifact verification, distribution, persisted settings and plugin management operations. Harmos SHALL expose no `PluginBackend` policy interface.

#### Scenario: Startup Loads Installed Entries

- **WHEN** a product starts with several installed plugins
- **THEN** its registry selects the entries and calls the typed facade to load each one, and Harmos performs no installed-entry discovery

### Requirement: Kind Admission Before Initialize

A product SHALL register each plugin kind with product admission and settings-resolution callbacks. Harmos SHALL validate descriptor closure, identities, fingerprints and declaration digests before product admission, resolve settings before initialize, and publish no usable activation until initialize succeeds.

#### Scenario: Product Rejects a Section

- **WHEN** generic declarations are valid but a kind's admission callback refuses a product section
- **THEN** initialize does not run, no activation is published and Harmos settles the transport and process resources it acquired

#### Scenario: Settings Resolution Is Cancelled

- **WHEN** the load is cancelled while its kind resolves settings
- **THEN** the callback receives cancellation under the load policy and the activation is not initialized or published

### Requirement: One Runtime Owner

Harmos SHALL own the plugin lifecycle state machine, process and Wasm hosting, transport, reconnect/fencing, operation routing, sampling/windows, deadlines and cancellation. Products SHALL receive opaque activation, binding and sampling handles through the typed in-process facade, never managers or engines.

#### Scenario: Stale Activation Handle

- **WHEN** a product uses a handle retained from a retired activation after replacement
- **THEN** Harmos refuses it rather than routing work into the replacement generation

### Requirement: Carrier Keeps Product Payloads Opaque

The carrier SHALL contain connection identity/revision, admission descriptors, invocation correlation and opaque payloads, cancellation, host-call frames, window/sample framing and opaque product sections. Product API messages and product policy SHALL remain outside the carrier; declaration digests SHALL stay generic and artifact digests SHALL stay with product bundles.

#### Scenario: Two Different Product Contracts

- **WHEN** two products use different declarations and host-call DTOs over the carrier
- **THEN** Harmos validates generic admission and transports their opaque payloads without acquiring either product's domain contract
