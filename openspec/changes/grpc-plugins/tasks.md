# Tasks

## 1. Contract

- [ ] 1.1 Settle the open points in `design.md` with the owner and record each outcome there.
- [ ] 1.2 Lock the source inventory: producers, consumers, tests, fixtures, examples and guidance.
  - `CONTRIBUTING.md` — Migrate native authoring/example/fixture instructions to SDK builder values, serve and managed/remote `plugin-grpc`; retain guest guidance until 7.1.
  - `Cargo.lock` — Migrate default native features and facade exports to the new SDK/placement; remove obsolete fixture/bench target selections as consumers move, retaining guest dependencies until 7.1.
  - `Cargo.toml` — Migrate default native features and facade exports to the new SDK/placement; remove obsolete fixture/bench target selections as consumers move, retaining guest dependencies until 7.1.
  - `README.md` — Migrate native authoring/example/fixture instructions to SDK builder values, serve and managed/remote `plugin-grpc`; retain guest guidance until 7.1.
  - `crates/harmos-macros/tests/artifact/main.rs` — Migrate native artifact/catalog checks to SDK catalog construction; delete removed plugin macro/grant refusal cases; retain guest/host macro coverage (outside this slice).
  - `crates/harmos-macros/tests/catalog/main.rs` — Migrate native artifact/catalog checks to SDK catalog construction; delete removed plugin macro/grant refusal cases; retain guest/host macro coverage (outside this slice).
  - `crates/harmos-macros/tests/fixtures/empty_grant.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until 7.1 (outside this slice).
  - `crates/harmos-macros/tests/fixtures/explicit_version_plugin.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until 7.1 (outside this slice).
  - `crates/harmos-macros/tests/fixtures/facade/lib.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until 7.1 (outside this slice).
  - `crates/harmos-macros/tests/fixtures/facade_runtime_catalogs.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until 7.1 (outside this slice).
  - `crates/harmos-macros/tests/fixtures/plugin_on_struct.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until 7.1 (outside this slice).
  - `crates/harmos-macros/tests/fixtures/plugin_setup_operations.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until 7.1 (outside this slice).
  - `crates/harmos-macros/tests/fixtures/synchronous_activation.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until 7.1 (outside this slice).
  - `crates/harmos-macros/tests/fixtures/synchronous_deactivation.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until 7.1 (outside this slice).
  - `crates/harmos-macros/tests/fixtures/undeclared_custom_observer_output.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until 7.1 (outside this slice).
  - `crates/harmos-macros/tests/fixtures/undeclared_observer_output.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until 7.1 (outside this slice).
  - `crates/harmos-macros/tests/fixtures/unknown_grant.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until 7.1 (outside this slice).
  - `crates/harmos-packaging/examples/fixture-driver.rs` — Collect native identity/catalogs with Describe and Stop and package without embedded framed inventory; retain guest packaging.
  - `crates/harmos-packaging/tests/build/capture.rs` — Migrate native fixture build/inspection consumers from embedded sidecar inventory to SDK Describe catalogs; retain guest packaging (packaging owner follow-up).
  - `crates/harmos-packaging/tests/integration/main.rs` — Migrate native fixture build/inspection consumers from embedded sidecar inventory to SDK Describe catalogs; retain guest packaging (packaging owner follow-up).
  - `crates/harmos-runtime/Cargo.toml` — Migrate default native features and facade exports to the new SDK/placement; remove obsolete fixture/bench target selections as consumers move, retaining guest dependencies until 7.1.
  - `crates/harmos-runtime/benches/guest/main.rs` — Keep for guests; imports support from benches/sidecar and must retain that support when native transport benches are removed.
  - `crates/harmos-runtime/benches/sidecar/commands.rs` — Delete framed-sidecar transport benchmark and fixture mechanics.
  - `crates/harmos-runtime/benches/sidecar/fixture.rs` — Keep guest-used benchmark support until guest consumers are moved; delete sidecar transport branches, preserving guest benchmark behaviour.
  - `crates/harmos-runtime/benches/sidecar/main.rs` — Delete framed-sidecar transport benchmark and fixture mechanics.
  - `crates/harmos-runtime/benches/sidecar/system.rs` — Delete framed-sidecar transport benchmark and fixture mechanics.
  - `crates/harmos-runtime/benches/sidecar/transfers.rs` — Keep guest-used benchmark support until guest consumers are moved; delete sidecar transport branches, preserving guest benchmark behaviour.
  - `crates/harmos-runtime/benches/streams/main.rs` — Delete framed native-consumption benchmark branches with the transport; preserve host-only stream measurements.
  - `crates/harmos-runtime/tests/application/macros.rs` — Migrate native registrations/catalog inspection and integration wiring to `Describe`/SDK builders; delete old plugin/observer/catalog attribute-macro assertions; retain host/guest coverage.
  - `crates/harmos-runtime/tests/fixtures/process_client.rs` — Delete graph/reload-plan fixtures with that feature; migrate retained nested-call and lifecycle behaviour to SDK-backed stories.
  - `crates/harmos-runtime/tests/fixtures/process_provider.rs` — Delete graph/reload-plan fixtures with that feature; migrate retained nested-call and lifecycle behaviour to SDK-backed stories.
  - `crates/harmos-runtime/tests/fixtures/stream_consumer.rs` — Migrate retained host-to-plugin listener consumption to protobuf SDK operations through `plugin-grpc`; delete framed-carrier mechanics.
  - `crates/harmos-runtime/tests/integration/main.rs` — Migrate native registrations/catalog inspection and integration wiring to `Describe`/SDK builders; delete old plugin/observer/catalog attribute-macro assertions; retain host/guest coverage.
  - `crates/harmos-runtime/tests/integration/report.rs` — Migrate native registrations/catalog inspection and integration wiring to `Describe`/SDK builders; delete old plugin/observer/catalog attribute-macro assertions; retain host/guest coverage.
  - `crates/harmos-runtime/tests/observers/cadence.rs` — Migrate retained observer sampling/settings/demand behaviour to host-tick `plugin_grpc` stories; delete native runner, cadence-report and publication-lease coverage.
  - `crates/harmos-runtime/tests/observers/execution.rs` — Migrate retained observer sampling/settings/demand behaviour to host-tick `plugin_grpc` stories; delete native runner, cadence-report and publication-lease coverage.
  - `crates/harmos-runtime/tests/observers/host_accessor.rs` — Migrate retained observer sampling/settings/demand behaviour to host-tick `plugin_grpc` stories; delete native runner, cadence-report and publication-lease coverage.
  - `crates/harmos-runtime/tests/observers/lifecycle.rs` — Migrate retained observer sampling/settings/demand behaviour to host-tick `plugin_grpc` stories; delete native runner, cadence-report and publication-lease coverage.
  - `crates/harmos-runtime/tests/observers/main.rs` — Migrate retained observer sampling/settings/demand behaviour to host-tick `plugin_grpc` stories; delete native runner, cadence-report and publication-lease coverage.
  - `crates/harmos-runtime/tests/observers/native.rs` — Migrate retained observer sampling/settings/demand behaviour to host-tick `plugin_grpc` stories; delete native runner, cadence-report and publication-lease coverage.
  - `crates/harmos-runtime/tests/observers/registration.rs` — Migrate retained observer sampling/settings/demand behaviour to host-tick `plugin_grpc` stories; delete native runner, cadence-report and publication-lease coverage.
  - `crates/harmos-runtime/tests/operations/wire/artifact.rs` — Migrate native call/contract coverage to protobuf SDK and `plugin-grpc`; delete removed native routing/token mechanics and keep guest cases.
  - `crates/harmos-runtime/tests/operations/wire/dynamic.rs` — Migrate native call/contract coverage to protobuf SDK and `plugin-grpc`; delete removed native routing/token mechanics and keep guest cases.
  - `crates/harmos-runtime/tests/operations/wire/host.rs` — Migrate native call/contract coverage to protobuf SDK and `plugin-grpc`; delete removed native routing/token mechanics and keep guest cases.
  - `crates/harmos-runtime/tests/operations/wire/mod.rs` — Migrate native call/contract coverage to protobuf SDK and `plugin-grpc`; delete removed native routing/token mechanics and keep guest cases.
  - `crates/harmos-runtime/tests/operations/wire/plugin.rs` — Migrate native call/contract coverage to protobuf SDK and `plugin-grpc`; delete removed native routing/token mechanics and keep guest cases.
  - `crates/harmos-runtime/tests/operations/wire/plugin_activation.rs` — Delete plugin graph, reload-plan and generation-token/stale-call fencing coverage with those features; preserve required guest behaviour until 7.1.
  - `crates/harmos-runtime/tests/operations/wire/runtime_defined.rs` — Migrate native call/contract coverage to protobuf SDK and `plugin-grpc`; delete removed native routing/token mechanics and keep guest cases.
  - `crates/harmos-runtime/tests/plugin/activation.rs` — Delete plugin graph, reload-plan and generation-token/stale-call fencing coverage with those features; preserve required guest behaviour until 7.1.
  - `crates/harmos-runtime/tests/plugin/main.rs` — Delete plugin graph, reload-plan and generation-token/stale-call fencing coverage with those features; preserve required guest behaviour until 7.1.
  - `crates/harmos-runtime/tests/plugin/owners.rs` — Delete plugin graph, reload-plan and generation-token/stale-call fencing coverage with those features; preserve required guest behaviour until 7.1.
  - `crates/harmos-runtime/tests/runtime/main.rs` — Migrate native registrations/catalog inspection and integration wiring to `Describe`/SDK builders; delete old plugin/observer/catalog attribute-macro assertions; retain host/guest coverage.
  - `crates/harmos-runtime/tests/runtime/plugin.rs` — Migrate native registrations/catalog inspection and integration wiring to `Describe`/SDK builders; delete old plugin/observer/catalog attribute-macro assertions; retain host/guest coverage.
  - `crates/harmos-runtime/tests/sidecar/admission.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/composition.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/consumption.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/dependency_routing.rs` — Delete with plugin graph/reload ordering, grants and provider selection; retain ordinary typed child-call coverage on `plugin_grpc`.
  - `crates/harmos-runtime/tests/sidecar/grants.rs` — Delete with plugin graph/reload ordering, grants and provider selection; retain ordinary typed child-call coverage on `plugin_grpc`.
  - `crates/harmos-runtime/tests/sidecar/graph.rs` — Delete with plugin graph/reload ordering, grants and provider selection; retain ordinary typed child-call coverage on `plugin_grpc`.
  - `crates/harmos-runtime/tests/sidecar/listeners.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/main.rs` — Migrate fixture registrations to SDK builder values and remove the old sidecar story/fixture root after its consumers move.
  - `crates/harmos-runtime/tests/sidecar/observer_demand.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/observer_hosts.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/operation_lifecycle.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/operations.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/process.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/publication_delivery.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/publications.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/routing.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/status.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/sidecar/streams.rs` — Delete framed operation-stream transport coverage after resolving the example streamed-operation contract; the new Invoke has no operation-stream frames.
  - `crates/harmos-runtime/tests/sidecar/visibility.rs` — Migrate retained lifecycle, call, failure and stream behaviour to one-journey `plugin_grpc` stories using the host virtual clock; delete assertions for removed mechanisms.
  - `crates/harmos-runtime/tests/simulation/main.rs` — Migrate native scripted outage/reconnect behaviour to SDK-backed `plugin-grpc` with the shared virtual clock; keep simulation replay/recovery guarantees.
  - `crates/harmos-runtime/tests/state/listeners.rs` — Migrate native plugin fixtures to `plugin-grpc`; retain host stream/history behaviour and guest fixtures without graph-order assumptions.
  - `crates/harmos-runtime/tests/state/main.rs` — Migrate native plugin fixtures to `plugin-grpc`; retain host stream/history behaviour and guest fixtures without graph-order assumptions.
  - `crates/harmos-runtime/tests/storage/operation_plugins.rs` — Migrate native plugin fixtures to `plugin-grpc`; retain host stream/history behaviour and guest fixtures without graph-order assumptions.
  - `crates/harmos-runtime/tests/streams/listeners.rs` — Migrate native plugin fixtures to `plugin-grpc`; retain host stream/history behaviour and guest fixtures without graph-order assumptions.
  - `crates/harmos-runtime/tests/streams/main.rs` — Migrate native plugin fixtures to `plugin-grpc`; retain host stream/history behaviour and guest fixtures without graph-order assumptions.
  - `crates/harmos-runtime/tests/streams/revisions.rs` — Migrate native plugin fixtures to `plugin-grpc`; retain host stream/history behaviour and guest fixtures without graph-order assumptions.
  - `crates/harmos-runtime/tests/support/fixtures.rs` — Migrate native fixture inspection/library selectors as their consumers move to SDK Describe; retain stamped guest artifacts and shared fixture verification.
  - `crates/harmos-runtime/tests/support/native_publications.rs` — Delete the fixture used only by the retired native publication transport benchmark; it has no guest consumer.
  - `crates/harmos-runtime/tests/support/native_streams.rs` — Migrate retained host-to-plugin listener consumption to protobuf SDK operations through `plugin-grpc`; delete framed-carrier mechanics.
  - `crates/harmos-runtime/tests/support/native_streams/plugin.rs` — Migrate retained host-to-plugin listener consumption to protobuf SDK operations through `plugin-grpc`; delete framed-carrier mechanics.
  - `crates/harmos-runtime/tests/support/operations_fixture.rs` — Migrate sidecar fixture/support consumers to `plugin-grpc` as their stories move; retain shared host/guest support.
  - `crates/harmos-runtime/tests/support/pending.rs` — Migrate sidecar fixture/support consumers to `plugin-grpc` as their stories move; retain shared host/guest support.
  - `crates/harmos-runtime/tests/support/process_graph_contract.rs` — Delete graph/reload-plan fixtures with that feature; migrate retained nested-call and lifecycle behaviour to SDK-backed stories.
  - `crates/harmos-runtime/tests/support/report.rs` — Migrate sidecar fixture/support consumers to `plugin-grpc` as their stories move; retain shared host/guest support.
  - `crates/harmos-runtime/tests/support/witness.rs` — Migrate sidecar fixture/support consumers to `plugin-grpc` as their stories move; retain shared host/guest support.
  - `crates/harmos-runtime/tests/system/cpp/main.rs` — Delete the old C ABI sidecar prototype story with that feature; SDK conformance replaces it (5.3/6.4).
  - `crates/harmos-runtime/tests/system/guest_contract/dependencies.rs` — Keep for guests, untouched in this slice; mixed native/guest dependency fixtures still use the old path and require coordinated migration in 7.1.
  - `crates/harmos-runtime/tests/system/guest_contract/dependency_admission.rs` — Keep for guests, untouched in this slice; mixed native/guest dependency fixtures still use the old path and require coordinated migration in 7.1.
  - `crates/harmos-runtime/tests/system/guest_contract/lifecycle.rs` — Keep for guests, untouched in this slice; mixed native/guest dependency fixtures still use the old path and require coordinated migration in 7.1.
  - `crates/harmos-runtime/tests/system/guest_contract/limits.rs` — Keep for guests, untouched in this slice; mixed native/guest dependency fixtures still use the old path and require coordinated migration in 7.1.
  - `crates/harmos-runtime/tests/system/guest_contract/mod.rs` — Keep for guests, untouched in this slice; mixed native/guest dependency fixtures still use the old path and require coordinated migration in 7.1.
  - `crates/harmos-runtime/tests/system/guest_contract/operations.rs` — Keep for guests, untouched in this slice; mixed native/guest dependency fixtures still use the old path and require coordinated migration in 7.1.
  - `crates/harmos-runtime/tests/system/guest_contract/publications.rs` — Keep for guests, untouched in this slice; mixed native/guest dependency fixtures still use the old path and require coordinated migration in 7.1.
  - `crates/harmos-runtime/tests/system/main.rs` — Migrate native system journey/fixture selection to managed `plugin-grpc`; delete reload-plan/token/lease assertions, preserve guest journeys and example results.
  - `crates/harmos-runtime/tests/system/schematic.rs` — Migrate native system journey/fixture selection to managed `plugin-grpc`; delete reload-plan/token/lease assertions, preserve guest journeys and example results.
  - `crates/harmos-runtime/tests/system/sidecar.rs` — Replace Schematic retained advice/analysis/transfer behaviour with examples/schematic/app/tests/native_plugin; delete graph reload, generation and framed endpoint checks.
  - `crates/harmos-sidecar/benches/transport/main.rs` — Delete the framed transport benchmark with that transport.
  - `crates/harmos-sidecar/tests/attach/main.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/attach/source_scenarios.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/fixtures/catalog_sidecar.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/fixtures/throughput_protocol.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/fixtures/throughput_sidecar.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/integration/main.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/kit/main.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/kit/task.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/system/environment.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/system/main.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/transport/capacity.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/transport/channels.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/transport/main.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/transport/measurement.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos-sidecar/tests/transport/publication.rs` — Delete with framed transport, handshake, binary/control carriers and the old kit; retained lifecycle/catalog behaviour is covered through SDK-backed `plugin-grpc` stories.
  - `crates/harmos/Cargo.toml` — Migrate default native features and facade exports to the new SDK/placement; remove obsolete fixture/bench target selections as consumers move, retaining guest dependencies until 7.1.
  - `crates/harmos/src/lib.rs` — Migrate default native features and facade exports to the new SDK/placement; remove obsolete fixture/bench target selections as consumers move, retaining guest dependencies until 7.1.
  - `docs/examples.md` — Migrate native authoring/example/fixture instructions to SDK builder values, serve and managed/remote `plugin-grpc`; retain guest guidance until 7.1.
  - `docs/product-guide.md` — Migrate native authoring/example/fixture instructions to SDK builder values, serve and managed/remote `plugin-grpc`; retain guest guidance until 7.1.
  - `docs/schematic.md` — Migrate native authoring/example/fixture instructions to SDK builder values, serve and managed/remote `plugin-grpc`; retain guest guidance until 7.1.
  - `docs/sidecar-cpp-prototype.md` — Delete the C ABI prototype guide with that feature; use the new C++ SDK guide in docs/tooling.md (5.3/6.4).
  - `docs/tooling.md` — Migrate native authoring/example/fixture instructions to SDK builder values, serve and managed/remote `plugin-grpc`; retain guest guidance until 7.1.
  - `examples/schematic/Cargo.lock` — Migrate the native consumer to `plugin-grpc` placement and protobuf contracts; preserve guest selection and the example journey.
  - `examples/schematic/Cargo.toml` — Migrate the native consumer to `plugin-grpc` placement and protobuf contracts; preserve guest selection and the example journey.
  - `examples/schematic/app/Cargo.toml` — Migrate the native consumer to `plugin-grpc` placement and protobuf contracts; preserve guest selection and the example journey.
  - `examples/schematic/app/src/app.rs` — Migrate the native consumer to `plugin-grpc` placement and protobuf contracts; preserve guest selection and the example journey.
  - `examples/schematic/app/src/document.rs` — Migrate the native consumer to `plugin-grpc` placement and protobuf contracts; preserve guest selection and the example journey.
  - `examples/schematic/app/src/services/guest.rs` — Keep for guests, untouched in this slice; guest protocol migration belongs to 7.1.
  - `examples/schematic/app/src/services/mod.rs` — Migrate the native consumer to `plugin-grpc` placement and protobuf contracts; preserve guest selection and the example journey.
  - `examples/schematic/app/src/services/sidecar.rs` — Migrate the native consumer to `plugin-grpc` placement and protobuf contracts; preserve guest selection and the example journey.
  - `examples/schematic/app/src/storage.rs` — Migrate native plugin-history consumers as needed for the new placement; preserve retained evidence and guest history.
  - `examples/schematic/cli/src/advanced.rs` — Migrate the native consumer to `plugin-grpc` placement and protobuf contracts; preserve guest selection and the example journey.
  - `examples/schematic/cli/src/main.rs` — Migrate the native consumer to `plugin-grpc` placement and protobuf contracts; preserve guest selection and the example journey.
  - `examples/schematic/cli/src/walkthrough.rs` — Migrate the native consumer to `plugin-grpc` placement and protobuf contracts; preserve guest selection and the example journey.
  - `examples/schematic/domain/Cargo.toml` — Migrate native contracts to SDK protobuf messages; keep existing guest contracts working without changing the guest crate.
  - `examples/schematic/domain/src/error.rs` — Migrate native shared model consumers to SDK protobuf data without changing guest model behaviour; keep guest-required representations until 7.1.
  - `examples/schematic/domain/src/lib.rs` — Migrate native shared model consumers to SDK protobuf data without changing guest model behaviour; keep guest-required representations until 7.1.
  - `examples/schematic/domain/src/model/mod.rs` — Migrate native shared model consumers to SDK protobuf data without changing guest model behaviour; keep guest-required representations until 7.1.
  - `examples/schematic/domain/src/model/schematic/mod.rs` — Migrate native shared model consumers to SDK protobuf data without changing guest model behaviour; keep guest-required representations until 7.1.
  - `examples/schematic/domain/src/model/schematic/progress.rs` — Migrate native contracts to SDK protobuf messages; keep existing guest contracts working without changing the guest crate.
  - `examples/schematic/domain/src/model/schematic/validated.rs` — Migrate native shared model consumers to SDK protobuf data without changing guest model behaviour; keep guest-required representations until 7.1.
  - `examples/schematic/domain/src/model/transfer.rs` — Migrate native shared model consumers to SDK protobuf data without changing guest model behaviour; keep guest-required representations until 7.1.
  - `examples/schematic/guest/Cargo.toml` — Keep for guests, untouched in this slice; guest protocol migration belongs to 7.1.
  - `examples/schematic/guest/build.rs` — Keep for guests, untouched in this slice; guest protocol migration belongs to 7.1.
  - `examples/schematic/guest/src/lib.rs` — Keep for guests, untouched in this slice; guest protocol migration belongs to 7.1.
  - `examples/schematic/operations/Cargo.toml` — Migrate native contracts to SDK protobuf messages; keep existing guest contracts working without changing the guest crate.
  - `examples/schematic/operations/src/directories/list_directories.rs` — Migrate native directory access to SDK request/response messages and an operation builder; preserve filesystem behaviour.
  - `examples/schematic/operations/src/directories/mod.rs` — Migrate native directory access to SDK request/response messages and an operation builder; preserve filesystem behaviour.
  - `examples/schematic/operations/src/lib.rs` — Migrate native contracts to SDK protobuf messages; keep existing guest contracts working without changing the guest crate.
  - `examples/schematic/operations/src/schematic/analyze.rs` — Keep streamed findings and cancellation as a native host operation; retain the existing guest operation.
  - `examples/schematic/operations/src/schematic/exchange.rs` — Keep snapshot input, findings and progress as a native host operation; retain the existing guest declaration.
  - `examples/schematic/operations/src/schematic/mod.rs` — Migrate native analysis/routing contracts to SDK messages and builder values; preserve guest contracts and algorithm results.
  - `examples/schematic/operations/src/schematic/propose.rs` — Migrate native analysis/routing contracts to SDK messages and builder values; preserve guest contracts and algorithm results.
  - `examples/schematic/operations/src/schematic/upload.rs` — Keep snapshot input, validation and progress as a native host operation; retain the existing guest declaration.
  - `examples/schematic/operations/src/schematic/validate.rs` — Migrate native analysis/routing contracts to SDK messages and builder values; preserve guest contracts and algorithm results.
  - `examples/schematic/operations/src/transfer/download.rs` — Keep native transfer download in the host; preserve guest transfer benchmarks.
  - `examples/schematic/operations/src/transfer/hash.rs` — Migrate native transfer contracts/handlers to SDK messages and builder values; retain guest contracts and unary transfer behaviour.
  - `examples/schematic/operations/src/transfer/mod.rs` — Migrate native transfer contracts/handlers to SDK messages and builder values; retain guest contracts and unary transfer behaviour.
  - `examples/schematic/operations/src/transfer/put.rs` — Migrate native transfer contracts/handlers to SDK messages and builder values; retain guest contracts and unary transfer behaviour.
  - `examples/schematic/operations/src/transfer/store.rs` — Migrate native transfer contracts/handlers to SDK messages and builder values; retain guest contracts and unary transfer behaviour.
  - `examples/schematic/operations/src/transfer/take.rs` — Migrate native transfer contracts/handlers to SDK messages and builder values; retain guest contracts and unary transfer behaviour.
  - `examples/schematic/operations/src/transfer/upload.rs` — Keep native transfer upload in the host; preserve guest transfer benchmarks.
  - `examples/schematic/sidecar/Cargo.toml` — Migrate to `harmos-sdk-grpc` builder values and managed serving; remove framed registration and benchmark-only transport setup.
  - `examples/schematic/sidecar/build.rs` — Migrate to `harmos-sdk-grpc` builder values and managed serving; remove framed registration and benchmark-only transport setup.
  - `examples/schematic/sidecar/src/benchmark.rs` — Migrate to `harmos-sdk-grpc` builder values and managed serving; remove framed registration and benchmark-only transport setup.
  - `examples/schematic/sidecar/src/lib.rs` — Migrate to `harmos-sdk-grpc` builder values and managed serving; remove framed registration and benchmark-only transport setup.
  - `examples/schematic/sidecar/src/main.rs` — Migrate to `harmos-sdk-grpc` builder values and managed serving; remove framed registration and benchmark-only transport setup.
  - `examples/schematic/sidecar/src/registration.rs` — Migrate to `harmos-sdk-grpc` builder values and managed serving; remove framed registration and benchmark-only transport setup.
  - `examples/schematic/storage/src/journal.rs` — Migrate native plugin-history consumers as needed for the new placement; preserve retained evidence and guest history.
  - `examples/schematic/storage/src/lib.rs` — Migrate native plugin-history consumers as needed for the new placement; preserve retained evidence and guest history.
  - `examples/schematic/storage/tests/journal/main.rs` — Migrate native plugin-history consumers as needed for the new placement; preserve retained evidence and guest history.
  - `examples/schematic/storage/tests/journal/sidecar.rs` — Migrate native plugin-history consumers as needed for the new placement; preserve retained evidence and guest history.
  - `mise.toml` — Migrate native plugin fixture/library/inspection/build selections to SDK and `plugin-grpc`; delete transport-benchmark selections; retain guest fixture builds and shipped executable paths.
  - `tools/project.toml` — Migrate native plugin fixture/library/inspection/build selections to SDK and `plugin-grpc`; delete transport-benchmark selections; retain guest fixture builds and shipped executable paths.
  - `tools/python/src/harmos_tools/build/__main__.py` — Migrate native plugin fixture/library/inspection/build selections to SDK and `plugin-grpc`; delete transport-benchmark selections; retain guest fixture builds and shipped executable paths.
  - `tools/python/src/harmos_tools/build/benchmark_fixtures.py` — Migrate native plugin fixture/library/inspection/build selections to SDK and `plugin-grpc`; delete transport-benchmark selections; retain guest fixture builds and shipped executable paths.
  - `tools/python/src/harmos_tools/build/fixtures.py` — Migrate native plugin fixture/library/inspection/build selections to SDK and `plugin-grpc`; delete transport-benchmark selections; retain guest fixture builds and shipped executable paths.
  - `tools/python/src/harmos_tools/package/rust.py` — Migrate native plugin fixture/library/inspection/build selections to SDK and `plugin-grpc`; delete transport-benchmark selections; retain guest fixture builds and shipped executable paths.
  - `tools/python/src/harmos_tools/prepare/tests/fixtures.py` — Migrate native plugin fixture/library/inspection/build selections to SDK and `plugin-grpc`; delete transport-benchmark selections; retain guest fixture builds and shipped executable paths.
  - `tools/python/src/harmos_tools/release/layout.py` — Migrate native plugin fixture/library/inspection/build selections to SDK and `plugin-grpc`; delete transport-benchmark selections; retain guest fixture builds and shipped executable paths.
  - `tools/python/src/harmos_tools/release/policy.py` — Migrate native plugin fixture/library/inspection/build selections to SDK and `plugin-grpc`; delete transport-benchmark selections; retain guest fixture builds and shipped executable paths.
  - `tools/python/src/harmos_tools/test/preflight.py` — Migrate native plugin fixture/library/inspection/build selections to SDK and `plugin-grpc`; delete transport-benchmark selections; retain guest fixture builds and shipped executable paths.

## 2. Protocol

- [x] 2.1 Add `crates/harmos-protocol` exposing the `harmos.v1` definitions as `harmos_protocol::v1`, with checked-in `proto/` files and generated Rust, and a drift check that needs no installed `protoc`.
- [x] 2.2 Write the behaviour spec: describe, activation, invoke frames, child calls, cancellation, failures and streams.
- [x] 2.3 Add the conformance suite with a reference host usable by non-Rust SDKs.

## 3. Host

- [x] 3.1 Add the managed placement: start the process, read the address it announces, dial and supervise it, and call `Stop` before closing.
- [x] 3.2 Add the remote placement: connect to a URL, reconnect, and expose the start time of each activation.
- [x] 3.3 Compute and admit the catalog digest; route child calls on the invoke call.
- [x] 3.4 Open and close `Publish` calls per declared stream, send ticks for interval streams and ingest samples.
- [x] 3.5 Lift the simulation clock into one clock per host with a monotonic and a date reading, and move every time read and wait in the runtime, storage, the client-facing gRPC layer and the built-in operations onto it.
- [x] 3.6 Serve `harmos.clock.sleep` and `harmos.clock.now`, and drive observer ticks from host timers.
- [x] 3.7 Add the virtual mode: its own timer queue, activity accounting for calls, ticks and host work, and a controller that advances while a condition is unmet, with simulation's existing tests kept passing.
- [x] 3.8 Reduce visibility to `Private` and `Public`.

## 4. Rust SDK

- [ ] 4.1 Move `crates/harmos-sdk` to `sdk/rust`, keeping its crate name, and serve `harmos.v1` from it in place of the framed wire.
- [ ] 4.2 Replace attribute-macro authoring with derives for data and builder values for plugins, operations, streams and observers, with a target-selected `serve`; delete the plugin, observer and catalog macros and their sidecar and guest variants.
- [ ] 4.3 Rebuild observers as functions that answer host ticks, route every SDK wait through the host clock, and share `publish_on` and its error type.
- [ ] 4.4 Migrate the examples and fixtures.
  - [x] 4.4.1 Migrate Schematic advice to SDK protobuf builders and managed placement, host analysis and transfer streaming, native packaging and benchmark selectors.
  - [x] 4.4.2 Migrate the stream-consumer fixture and its host-row journey to SDK unary calls on managed placement.
  - [ ] 4.4.3 Retire or migrate crates/harmos-sidecar/tests/fixtures/catalog_sidecar.rs, crates/harmos-runtime/tests/fixtures/process_client.rs and process_provider.rs, and the native macro fixture consumers under crates/harmos-macros/tests; authoring fixture libraries now select harmos_sdk_grpc instead of harmos_sidecar.

## 5. Pilot SDKs

- [x] 5.1 Python: `sdk/python` over `harmos.v1`, passing the conformance suite.
- [x] 5.2 C#: `sdk/csharp` over `harmos.v1`, passing the conformance suite.
- [x] 5.3 C++: `sdk/cpp` over `harmos.v1`, passing the conformance suite.

## 6. Removal

- [ ] 6.1 Delete the framed wire, handshake, credits, control pipe and binary carriers.
- [ ] 6.2 Delete publication leases, admission, cadence reports and the native observer runner.
- [ ] 6.3 Delete the plugin graph, start ordering and reload plans; grants and provider selection; generation tokens and stale-call fencing.
- [ ] 6.4 Delete `crates/harmos-ffi` and the C ABI bridge under `sdk/cpp`.
- [ ] 6.5 Derive protobuf definitions from operation types into the catalog and delete the Postcard message encoding on the wire.
- [ ] 6.6 Migrate the runtime sidecar behaviour tests and delete the transport tests and benches.
  - [x] 6.6.1 Complete targeted proof of the replacement Schematic composition and host-row consumption journeys.
  - [ ] 6.6.2 Migrate the remaining retained behaviour under crates/harmos-runtime/tests/{sidecar,observers,operations/wire,application,plugin,runtime,integration} and remove assertions for retired graph/grant/token/lease/cadence mechanisms.
  - [x] 6.6.3 Delete framed transport tests and benches in crates/harmos-sidecar and native transport benchmark branches in crates/harmos-runtime/benches, preserving guest-used support.
- [ ] 6.7 Update the product guide, README and authoring guidance.

## 7. Guests

- [ ] 7.1 Move guest command and child envelopes to the `harmos.v1` messages and delete the Postcard envelope codec.

## 8. Final Validation

- [ ] 8.1 Run targeted transport, lifecycle, conformance and guest tests on Linux and Windows.

## Workflow Follow-Up

- Dokime adopts the merged revision in its own `grpc-plugins` change.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
- SDK system tests run in Nightly only, not on pull requests.
