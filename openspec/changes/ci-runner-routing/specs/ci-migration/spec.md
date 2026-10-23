## ADDED Requirements

### Requirement: Project-Owned Runner Routing

CI SHALL select runner labels from project configuration and use hosted defaults for jobs without a local override. Both environments SHALL execute identical required validation tasks.

#### Scenario: Heavy Windows Build

- **WHEN** the Windows Product build is rendered
- **THEN** it selects the explicitly declared local Windows runner and preserves its required checks and artifact outputs

#### Scenario: Lightweight Repository Check

- **WHEN** the repository check has no local override
- **THEN** it runs on the declared hosted default without consulting provider runner variables
