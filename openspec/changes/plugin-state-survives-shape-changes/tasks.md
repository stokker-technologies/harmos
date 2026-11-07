## Implementation

- [x] 1.1 Trace persistent state ownership, journal and snapshot decoding, plugin scope inventories and object retention; lock the source inventory and acceptance.
- [ ] 1.2 Persist and restore plugin fact definitions through the existing journal and snapshot payload codec; keep host catalog refusal.
- [ ] 1.3 Preserve object definitions absent from the current catalog during retention and use existing historical stored-schema readers.
- [ ] 1.4 Add deterministic one-root shape-change/absence tests for records, facts and objects, with journal/snapshot and host refusal proof.
- [ ] 1.5 Update affected README and product guide contracts and consumer adoption instructions.

## Validation and Retirement

- [ ] 2.1 Run focused acceptance and affected crate tests through mise.
- [ ] 2.2 Build workspace test targets through build:rust:tests and run lint:rust; repair failures.
- [ ] 2.3 Validate and retire this OpenSpec change after source review and the required clean diff scan.
