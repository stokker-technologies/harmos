## Implementation

- [x] 1.1 Remove impact planning and its dependency graph; run every applicable PR job while preserving caches and platform matrices.
- [x] 1.2 Restrict release dry validation and git dependency rejection to releases; retain nightly warning behavior.
- [x] 1.3 Add exact-commit required-check API verification before release tag and publication.
- [x] 1.4 Add serialized nightly publication and atomic publisher for all cataloged distribution assets.
- [x] 1.5 Add focused tooling tests and the mise test entry point; cover dev/PR job selection without an impact planner.
- [x] 1.6 Update CONTRIBUTING and existing CI ownership contracts; review README impact and baseline failures.

## Validation and Delivery

- [ ] 2.1 Run the combined repository check after the task-model and generated-source checker defects are resolved.
- [ ] 2.2 Push the combined candidate and verify the remote SHA. Run targeted tasks locally; hosted CI and Nightly provide full validation.
- [ ] 2.3 Update the combined review to ready when authorized, report its CI once and leave it unmerged.
- [x] 2.3.1 Open the integration branch review into dev.

## Workflow Follow-Up

- Specification retirement follows review and resolution of required proof; this task does not authorize integration.
