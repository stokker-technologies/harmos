# ci-migration Specification

## Purpose
Preserve Harmos native platform coverage and release behavior through project-owned workflows, actions and tooling, initially seeded from editable Hub baselines.

## Requirements

### Requirement: Adopt The Agreed Hub Contract
The product SHALL own its tasks, configuration, tooling and workflows while preserving required native platform coverage, artifact dependencies and release behavior. Ordinary Hub updates SHALL leave CI untouched; only explicit `stkr project ci render` SHALL overwrite CI baselines.

#### Scenario: Candidate Delivery
- **WHEN** complete source passes applicable local proof
- **THEN** publish the candidate PR to dev, monitor and repair required provider checks until green, merge with protections enforced and verify the merged revision.

### Requirement: Honest Baseline And Repairs
The initial full baseline SHALL run before implementation as explicitly requested; every observed existing failure remains actionable and cannot waive final gates. Only targeted proof runs during migration until source closes. Optional nightly workloads are excluded unless explicitly requested.

#### Scenario: Baseline Failure
- **WHEN** an initial lane fails or a prerequisite prevents it running
- **THEN** record the actual result, repair the cause within the authorized migration and prove required behavior before delivery.

### Requirement: Project-Owned Runner Routing

CI SHALL select runner labels from project configuration and use hosted defaults for jobs without a local override. Both environments SHALL execute identical required validation tasks.

#### Scenario: Heavy Windows Build

- **WHEN** the Windows Product build is selected
- **THEN** it uses the declared GitHub-hosted Windows runner and preserves its required checks and artifact outputs

#### Scenario: Lightweight Repository Check

- **WHEN** the repository check has no local override
- **THEN** it runs on the declared hosted default without consulting provider runner variables
