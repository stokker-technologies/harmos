## Purpose

Define which operations leave the host process: Harmos host operations stay in-process, products expose them only through their own wrapper operations, and the plugin protocol stays on the wire.

## ADDED Requirements

### Requirement: Host Operations Are In-Process Only

Every Harmos host operation SHALL be registered in-process only, carry no gRPC exposure marker and have no listener binding. Client calls, encoded invocations and plugin child requests naming a Harmos host operation SHALL be refused as unknown before any handler runs.

#### Scenario: Client Calls a Harmos Host Operation

- **WHEN** a client calls a Harmos host method on the listener
- **THEN** the host refuses the call as unimplemented and executes no handler

#### Scenario: Plugin Child Request Names a Harmos Host Operation

- **WHEN** a plugin sends a child request naming a Harmos host operation
- **THEN** the host refuses it as unknown with the existing structured failure

### Requirement: Product Wrappers Expose Operations

A product SHALL expose a Harmos host operation only by declaring its own wrapper operation that calls it in-process. The wrapper's own declared audience SHALL govern client and plugin admission and SDK projection.

#### Scenario: Wrapped Plugin Listing

- **WHEN** a product declares a client wrapper that calls the in-process plugin listing operation
- **THEN** clients reach the wrapper, plugins are refused, and the product's generated client contains the wrapper only

### Requirement: No Caller Narrowing Input

The runtime and generators SHALL NOT accept a composition or caller narrowing input or exposure filters. Generators SHALL derive client and plugin projections from the product's descriptors and the audience declared on the product's own methods.

#### Scenario: Generation Without Composition

- **WHEN** a product generates SDK bindings from its embedded descriptors
- **THEN** the output contains its own wrappers for each declared audience and no Harmos host resource

### Requirement: Plugin Protocol Stays on the Wire

The plugin carrier SHALL keep instance, invocation, window, sample and child frames, plugin-declared operations and their Private, Protected and Public visibility, and a fixed set of plugin host facilities for time and the plugin's own settings.

#### Scenario: Plugin Reads Time

- **WHEN** a plugin asks its context for the host time or sleeps on the host clock
- **THEN** the request uses the plugin protocol facility and needs no product wrapper

#### Scenario: Public Peer Requirement

- **WHEN** a plugin calls a Public operation of another plugin under a declared requirement
- **THEN** routing, fingerprints, scope inheritance and cancellation behave as before this change

### Requirement: Discovery Matches Dispatch

In-process operation discovery SHALL apply the same admission predicate as dispatch for the caller it serves, including scoped plugin ownership, visibility and declared requirements.

#### Scenario: Wrapped Discovery for a Plugin

- **WHEN** a product wrapper forwards a plugin's discovery request
- **THEN** the result omits every entry that child dispatch would refuse for that plugin
