# Resource Store Specification

## Purpose

Define shared lifecycle setup, frozen resource ownership and constant catalog inventory for Harmos applications and plugins.

## Requirements

### Requirement: Shared Lifecycle

Applications and sidecars SHALL implement one Lifecycle contract with Config,
Error and Context, asynchronous prepare(config, setup), initialize(context), stop(context)
and cleanup(). Application SHALL extend Lifecycle and retain its non-resource type items.

#### Scenario: Application Order

- **WHEN** an application launches, serves and shuts down
- **THEN** its original owner runs prepare, initialize, serve, drain, stop and cleanup in order.

#### Scenario: Plugin Order

- **WHEN** a sidecar activates and stops
- **THEN** its original owner follows the same prepare, initialize, serve, stop, cleanup order.

### Requirement: Single Setup And Frozen Resources

Setup SHALL be the single registration surface, with mutable resource insertion and, only
for applications, lane settings and operation placement. A::Lanes SHALL declare itself.
Runtime SHALL launch through launch(origin,config); builder SHALL NOT be public. One
allocation per type SHALL resolve through ctx.resource in all operation kinds.

#### Scenario: Test Resource Override

- **WHEN** launch_with replaces a prepared resource by type
- **THEN** the normal production preparation and operation wiring use that fake allocation.

#### Scenario: Frozen Dynamic Validation

- **WHEN** dynamic registration requires an unavailable resource
- **THEN** it fails before publishing or starting work against the frozen store.

### Requirement: Aggregate Setup Errors

Duplicate resource types, duplicate catalogs, missing declared operation resources and
unknown lanes SHALL be typed errors, collected and reported together when prepare returns.
Invalid setup SHALL NOT initialize or serve.

#### Scenario: Multiple Setup Errors

- **WHEN** prepare records duplicate resources/catalogs, missing resources and unknown lanes
- **THEN** launch reports every corresponding error and initialize is never called.

#### Scenario: Invalid Setup Cleanup

- **WHEN** prepare returns an owner but setup validation fails
- **THEN** its cleanup releases prepared resources without initialize or stop, and failed cleanup retains that owner for retry.

### Requirement: Constant Plugin Catalogs

Plugin CATALOGS SHALL own compile-time catalog selection for both inert inventory and
execution. Plugins SHALL NOT call setup.operations. The provides attribute argument,
bind_operations, resource bundles and consumer RegistrationBatch APIs SHALL be removed.

#### Scenario: Inert Catalog Inspection

- **WHEN** packaging inspects a plugin with CATALOGS and feature-selected entries
- **THEN** it reads the matching catalog inventory without executing lifecycle methods.

### Requirement: Symmetric Facade Attributes

The harmos facade SHALL expose the sidecar module and attribute behind matching
features. Generated paths SHALL resolve through the facade without caller-ending guesses;
resource and requirement declarations SHALL NOT be silently cfg-dropped.

#### Scenario: Facade-Only Consumer

- **WHEN** a consumer depends on harmos and uses the facade attribute
- **THEN** it compiles and embeds the expected inventory without direct carrier dependencies.

### Requirement: Lifecycle Guidance

Maintained guidance SHALL show application prepare Resources/Lanes/Operations sections,
plugin CATALOGS, fake injection, lifecycle-owned resource close and distinct newtypes for
multiple allocations of one implementation type. Resource stop callback APIs SHALL be absent.

#### Scenario: Consumer Authoring

- **WHEN** a consumer follows the authoring skill or product guide
- **THEN** it uses the same lifecycle and setup contract as the shipped examples.
