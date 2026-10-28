## Extended Case Partition

The previous 900-case selection partitions into 637 canonical cases and the following 263 cases selected by the existing noncanonical feature guards. A namespace ending in `::*` includes every existing testcase below that namespace; individual entries identify one testcase. These cases remain in `test:rust:extended`, with their original assertions. No pre-existing canonical case is removed; new default-profile regression stories do not alter this feature partition.

| Binary | Namespace Or Case | Cases |
|---|---|---|
| `harmos-runtime` | `operations::*` | 1 |
| `harmos-runtime::acceptance` | `storage::*` | 1 |
| `harmos-runtime::integration` | `application::macros::unified_plugins::*` | 5 |
| `harmos-runtime::integration` | `observers::execution::*` | 8 |
| `harmos-runtime::integration` | `observers::host_accessor::*` | 3 |
| `harmos-runtime::integration` | `observers::lifecycle::*` | 11 |
| `harmos-runtime::integration` | `observers::native::*` | 6 |
| `harmos-runtime::integration` | `operations::execution::reboot_reruns_initialize` | 1 |
| `harmos-runtime::integration` | `operations::execution::simulation_reboot_rebinds` | 1 |
| `harmos-runtime::integration` | `operations::wire::artifact::*` | 2 |
| `harmos-runtime::integration` | `operations::wire::authored::response_awaits_cleanup` | 1 |
| `harmos-runtime::integration` | `operations::wire::authored::response_drop_retains` | 1 |
| `harmos-runtime::integration` | `operations::wire::authored::response_keeps_owner` | 1 |
| `harmos-runtime::integration` | `operations::wire::dynamic::*` | 2 |
| `harmos-runtime::integration` | `operations::wire::endpoints::deadline_settles_peer` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::exchange_drop_cancels` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::failure_preserves_cleanup` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::failure_retains_cleanup` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::grpc_exchange_polls` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::half_close_joins` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::input_drop_cancels` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::opening_admission_exchange` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::settlement_never_wait` | 1 |
| `harmos-runtime::integration` | `operations::wire::grpc::*` | 8 |
| `harmos-runtime::integration` | `operations::wire::grpc_manager::*` | 15 |
| `harmos-runtime::integration` | `operations::wire::host::*` | 2 |
| `harmos-runtime::integration` | `operations::wire::plugin::*` | 15 |
| `harmos-runtime::integration` | `plugin::*` | 26 |
| `harmos-runtime::integration` | `runtime::plugin::*` | 6 |
| `harmos-runtime::integration` | `sidecar::*` | 72 |
| `harmos-runtime::integration` | `simulation::*` | 15 |
| `harmos-runtime::integration` | `state::listeners::*` | 8 |
| `harmos-runtime::integration` | `storage::operations::plugins::*` | 2 |
| `harmos-runtime::integration` | `streams::listeners::*` | 9 |
| `harmos-runtime::system` | `guest::*` | 1 |
| `harmos-runtime::system` | `guest_contract::*` | 30 |
| `harmos-runtime::system` | `sidecar::*` | 1 |
