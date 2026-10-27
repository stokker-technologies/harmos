# Executable Operation Specification

## Purpose
Define portable operation requests and feature-gated execution in one vertical file.

## Requirements

### Requirement: Flat Feature-Gated Operation Authoring
An operation SHALL be authored vertically in one file. Its definition division SHALL contain messages, the request operation struct,
policy, contract implementations and contract tests. A runtime division SHALL follow the definition Tests section, containing a
feature-gated runtime module with its own complete skeleton and runtime-only tests. Live resources SHALL be declared separately from
request fields and accessed through Context.

#### Scenario: Contract-Only Consumer
- **WHEN** a consumer builds without its runtime feature
- **THEN** messages, definitions and catalogs remain available without optional execution dependencies

#### Scenario: Execution Consumer
- **WHEN** the consumer enables runtime
- **THEN** the same operation request implements Operation in its runtime module

### Requirement: Consuming Execution And Shared Definition
Authors SHALL implement Operation with an error associated type and consuming run method. The macro SHALL use the struct as its request,
default response to (), derive StreamEndpoints and expose its shared OperationDefinition. Each attempt SHALL execute a clone of the
original request. Carrier lifecycle state SHALL remain under the distinct Handler contract.

#### Scenario: Definition Without Implementation
- **WHEN** Read::definition() is used in a contract-only build
- **THEN** it returns portable metadata without needing execution resources

#### Scenario: Mismatched Response
- **WHEN** run returns a response inconsistent with the declaration
- **THEN** compilation fails at the implementation boundary
