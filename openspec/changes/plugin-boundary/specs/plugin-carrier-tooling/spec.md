## Purpose

Reuse carrier behavior and descriptor generation across products without requiring library types in their author-facing SDKs.

## ADDED Requirements

### Requirement: Four-Language Carrier Helpers

Harmos SHALL supply thin Rust, Python, C# and C++ helpers for carrier transport, cancellation and windows, retaining guest support where implemented. Products SHALL own the author-facing SDKs; no author SHALL need a Harmos type, namespace, attribute, exception or escape hatch. Import aliases SHALL NOT be used to conceal this boundary.

#### Scenario: Product Author Uses Windows

- **WHEN** an author implements a window observer through a product SDK
- **THEN** its public signature uses product API types while the implementation delegates window framing and cancellation to Harmos helpers

### Requirement: One Shared Generator

Harmos SHALL own reusable descriptor/codegen tooling for all four languages. Consolidation SHALL preserve the consuming product's richer renderers and fixes, including presence, nested values, language keywords, enum/repeated values, typed resources and call options, without depending on product packages.

#### Scenario: Product Generates Its Context

- **WHEN** a product supplies descriptors, namespace and output locations to the shared generator
- **THEN** it receives its typed resources with those descriptor semantics preserved and needs no forked renderer

### Requirement: Generic Conformance Has No Product Policy

The Harmos conformance kit SHALL cover generic admission, lifecycle, routing, transport, fencing, cancellation and windows using fixture product contracts. Product admission, installation, settings persistence and domain behavior SHALL remain product conformance responsibilities.

#### Scenario: Product Reuses the Kit

- **WHEN** a product verifies its four author SDKs
- **THEN** it reuses the carrier kit and adds its own API and policy proof without cloning the generic suite or adding product kinds to Harmos
