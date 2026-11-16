## Purpose

Define inert, frozen declaration composition through product-owned registrars and module catalogs, backed internally by reusable Harmos accumulation mechanics in four languages.

## ADDED Requirements

### Requirement: Register Phase on a Registrar

Each product SDK SHALL own the registrar exposed to authors and compose declarations through internal Harmos carrier helpers. No Harmos registrar or registration type SHALL leak to authors. The call SHALL be register in Rust/Python, Register in C# and add in C++, where register is reserved.

#### Scenario: Register a Module Catalog

- **WHEN** an author's register function registers the settings, operations and streams catalogs
- **THEN** the plugin's declarations contain every setting, operation and stream from those catalogs

### Requirement: Typed Registrations Accumulate

A typed registration SHALL be checked at the call and its descriptors accumulated by the registrar, so the plugin value keeps one type regardless of how many typed records, messages or kinds are registered.

#### Scenario: Many Typed Records

- **WHEN** an author registers ten typed record contracts
- **THEN** the plugin compiles with the same plugin type and its declarations carry all ten descriptors

### Requirement: Registration Is Inert

Registration SHALL construct declaration values only. It SHALL run no handler, open no connection, start no task and acquire no resource; any such work SHALL run in initialize.

#### Scenario: Device Construction

- **WHEN** a plugin needs a device handle for its operations
- **THEN** registration declares the operations and initialize opens the device

### Requirement: Declarations Frozen Before Serving

Registration SHALL complete before the plugin serves. The plugin SHALL report the frozen declarations through the existing declarations call, and the host SHALL treat them as immutable for the activation.

#### Scenario: Registration Error

- **WHEN** an author registers two operations with the same identity
- **THEN** the plugin fails to start with the duplicate identity named, before serving any connection

### Requirement: Module Catalogs

Each module that contributes declarations SHALL expose them through a function named catalog (Catalog in C#). The frozen host-facing description SHALL remain declarations.

#### Scenario: Example Plugins

- **WHEN** a maintained example plugin is read
- **THEN** each contributing module exposes catalog and the root register function registers each catalog once

### Requirement: Product Kinds Through Declaration Sections

A product SDK SHALL register its own declaration kinds through its public registrar, translating internally into opaque typed declaration sections. Harmos carrier helpers SHALL contain no product kind or admission policy; the product kind's admission callback SHALL run after generic validation and before initialize.

#### Scenario: Product Section

- **WHEN** a product kind is registered through the registrar
- **THEN** its entries appear in the product's declaration section of the frozen declarations and the host admits them through the product's checks
