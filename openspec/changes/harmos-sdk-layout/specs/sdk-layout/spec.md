## ADDED Requirements

### Requirement: One SDK Folder
Each language's plugin interfaces SHALL live in `sdk/<language>/`.

#### Scenario: Finding an SDK
- **WHEN** a developer looks for the Rust, C++ or Go interfaces
- **THEN** each is under its language folder in `sdk/`

#### Scenario: Move Without Behaviour Change
- **WHEN** the Rust crate moves
- **THEN** its crate name and public API are unchanged
