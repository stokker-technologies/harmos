# Consumer Guidance

## ADDED Requirements

### Requirement: Self-Contained Consumer Skills

The skills SHALL guide products using Harmos, including shipped consumer examples, without requiring company skills or repository-specific CI machinery.

#### Scenario: External Consumer

- **WHEN** an agent reads a shipped skill without global company guidance
- **THEN** it can determine applicable ownership and authoring rules from that skill and its linked references

### Requirement: Product Ownership And Composition

The guidance SHALL expose the domain independently through the entry crate and gate runtime exports behind runtime. Reusable operation types and implementations SHALL share an operations crate, with execution behind runtime and shared models in domain, respecting Rust's orphan rule. Self-contained operations SHALL colocate declarations and handlers when no independent contract consumer exists.

#### Scenario: Composed Plugin

- **WHEN** a plugin hosts reusable product execution
- **THEN** it consumes the entry crate, advertises the original operation contracts, enables their compatible implementations, shares initialized resources and owns lifecycle settlement

#### Scenario: Portable Plugin Inventory

- **WHEN** a plugin supplies platform-specific implementations of the same operations
- **THEN** it gates native implementation modules and retains one ungated catalog inventory across supported platforms

#### Scenario: Platform-Only Application Operation

- **WHEN** an application exposes an operation on only one platform
- **THEN** matching module and catalog variant gates retain only that platform's portable declarations and executable registrations

#### Scenario: Self-Contained Operation

- **WHEN** an operation is private to a self-contained interface
- **THEN** its declaration and handler share a vertical operation module without a mandatory local domain tree

### Requirement: Consumer Workspace Membership

The guidance SHALL place product crates and a single sidecar/guest under crates/, with multiple carrier plugins under plugins/sidecars/<name> and plugins/guests/<name> as publish = false workspace members.

#### Scenario: Several Plugins

- **WHEN** a product has multiple sidecar or guest crates
- **THEN** its workspace includes the populated plugin paths alongside crates/* and each carrier declares publish = false

### Requirement: Accurate Boundaries And Examples

The guidance SHALL distinguish portable configuration from resources, external resource libraries from product interfaces, native and guest capabilities, and consumer tests from Harmos maintenance. Examples SHALL use generic product names and accurately distinguish illustrative consumer layout from landed source.

#### Scenario: Resource And Guest Reuse

- **WHEN** a product integrates external clients or a guest
- **THEN** the guidance preserves cohesive client boundaries and requires compatible guest dependencies and contexts rather than assuming native execution portability
