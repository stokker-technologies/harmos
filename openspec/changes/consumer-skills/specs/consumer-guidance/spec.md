# Consumer Guidance

## ADDED Requirements

### Requirement: Self-Contained Consumer Skills

The skills SHALL guide products using Harmos, including shipped consumer examples, without requiring company skills or repository-specific CI machinery.

#### Scenario: External Consumer

- **WHEN** an agent reads a shipped skill without global company guidance
- **THEN** it can determine applicable ownership and authoring rules from that skill and its linked references

### Requirement: Product Ownership And Composition

The guidance SHALL place product and interface crates under crates/, expose the domain independently through the entry crate, gate runtime exports behind runtime, and separate reusable declarations from implementations. Self-contained operations SHALL colocate declarations and handlers when no independent contract consumer exists.

#### Scenario: Composed Plugin

- **WHEN** a plugin hosts reusable product execution
- **THEN** it consumes the entry crate, advertises original domain contracts, binds compatible runtime handlers, shares initialized resources and owns lifecycle settlement

#### Scenario: Self-Contained Operation

- **WHEN** an operation is private to a self-contained interface
- **THEN** its declaration and handler share a vertical operation module without a mandatory local domain tree

### Requirement: Accurate Boundaries And Examples

The guidance SHALL distinguish portable configuration from resources, external resource libraries from product interfaces, native and guest capabilities, and consumer tests from Harmos maintenance. Examples SHALL use generic product names and accurately distinguish illustrative consumer layout from landed source.

#### Scenario: Resource And Guest Reuse

- **WHEN** a product integrates external clients or a guest
- **THEN** the guidance preserves cohesive client boundaries and requires compatible guest dependencies and contexts rather than assuming native execution portability
