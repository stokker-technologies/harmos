# Tasks

## Nightly Repairs After Protocol Layout

- [x] 10.1 Refresh the standalone Python SDK lock through its prepare task and make local Python checks validate the same locked environment as CI. Own `sdk/python/mise.toml`, `sdk/python/uv.lock`, root Python task dispatch and `docs/tooling.md`. The SDK lock omitted the referenced tooling package's added formatter metadata; `--no-sync` skipped freshness validation.
- [x] 10.2 Match benchmark object schemas to the registered catalog and configure and start the guest benchmark's plugin entry manager and declared host file operations. Own storage benchmark workload/setup and runtime guest benchmark fixtures; preserve production admission checks. Derive the workload key from the registered Sample definition instead of hardcoding revision one; storage interface, baseline and lag workloads pass. The guest fixture explicitly starts the manager against its shared runtime before registration, matching the working guest stories; focused guest compilation passes and final execution remains under 10.4.
- [x] 10.3 Migrate release protocol inputs and archive fixtures to the model/host/plugin layout; search packaging, release and documentation tooling for retired paths. Own release staging and its tooling stories. The archive story covers all three trees and both channels and passes; the retired-path search is clean.
- [ ] 10.4 On the committed candidate, run Windows format, language/repository checks, locked workspace/tests/benches compilation, CI's Python prepare, all storage/query/runtime benches, language test lanes, packaging and credential-free release rehearsal. Repair discovered failures; retain native Linux and live publication proof as hosted obligations. Commits stay local.

## Caller Surfaces and Refusal Repairs

- [x] 9.1 Generate client and plugin surfaces from effective composition callers in all four languages; prove narrowing with the shared SDK fixture and negative consumers. Own `tools/python`, `sdk` generators and fixtures.
- [x] 9.2 Hide wire identities behind internal SDK modules and deliberate barrels; enforce public headers, stubs, exports and guides in the repository check. Own `sdk`, `tools/python`, `mise.toml` and affected guides.
- [x] 9.3 Repair reflection descriptor composition and Python canonical service registration; call every SDK through canonical gRPC paths in conformance. Own runtime gRPC descriptors, protocol generation and `tools/conformance`.
- [x] 9.4 Define wrong-caller failure once and preserve kind, code, action and operation across direct, child, tick, guest and nested calls; strengthen both caller conformance rules and controlled-clock stories. Own `crates/harmos-operations`, `crates/harmos-runtime`, `tools/conformance` and SDK fixtures. Supply the shared guest fixture's new clock requirement grant to all of its story consumers and retain existing child operation frames.
- [ ] 9.5 On the committed candidate, run format, each language check, repository check, locked workspace/tests compilation, four SDK conformance fixtures, runtime system stories, SDK build/tests and packaged consumers; verify regeneration leaves a clean tree. Native Linux proof remains required under 8.1 and needs a native Linux development environment. Commits remain local; delivery is not authorized.

## 1. Contract

- [ ] 1.1 Settle the open points in `design.md` with the owner and record each outcome there.
- [x] 1.2 Lock the source inventory: producers, consumers, tests, fixtures, examples and guidance. The indented list records the completed native cutover, including historical paths and feature names; remaining crate consolidation is owned by 4.7, 6.5, 6.8–6.12 and 7.1.
  - `docs/CONTRIBUTING.md` — Migrate native authoring/example/fixture instructions to SDK builder values, serve and managed/remote `plugin-grpc`; retain guest guidance until 7.1.
  - `Cargo.lock` — Migrate default native features and facade exports to the new SDK/placement; remove obsolete fixture/bench target selections as consumers move, retaining guest dependencies until 7.1.
  - `Cargo.toml` — Migrate default native features and facade exports to the new SDK/placement; remove obsolete fixture/bench target selections as consumers move, retaining guest dependencies until 7.1.
  - `README.md` — Migrate native authoring/example/fixture instructions to SDK builder values, serve and managed/remote `plugin-grpc`; retain guest guidance until 7.1.
  - `crates/harmos-macros/tests/artifact/main.rs` — Replace native embedded-catalog assertions with SDK catalog construction; retain guest embedded-metadata assertions.
  - `crates/harmos-macros/tests/catalog/main.rs` — Retire native plugin/observer macro refusal cases and inspect guest/host catalogs; retain guest-required macro diagnostics until 7.1.
  - `crates/harmos-macros/tests/fixtures/empty_grant.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until `authoring-without-macros` replaces their consumers.
  - `crates/harmos-macros/tests/fixtures/explicit_version_plugin.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until `authoring-without-macros` replaces their consumers.
  - `crates/harmos-macros/tests/fixtures/facade/lib.rs` — Remove the native sidecar re-export; retain guest and host macro-path coverage.
  - `crates/harmos-macros/tests/fixtures/facade_runtime_catalogs.rs` — Remove native plugin/catalog fixture declarations; retain guest and host catalog inspection.
  - `crates/harmos-macros/tests/fixtures/plugin_on_struct.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until `authoring-without-macros` replaces their consumers.
  - `crates/harmos-macros/tests/fixtures/plugin_setup_operations.rs` — Delete the retired native plugin setup macro refusal fixture.
  - `crates/harmos-macros/tests/fixtures/synchronous_activation.rs` — Delete the retired native observer activation macro refusal fixture.
  - `crates/harmos-macros/tests/fixtures/synchronous_deactivation.rs` — Delete the retired native observer deactivation macro refusal fixture.
  - `crates/harmos-macros/tests/fixtures/undeclared_custom_observer_output.rs` — Delete the retired custom observer output macro refusal fixture.
  - `crates/harmos-macros/tests/fixtures/undeclared_observer_output.rs` — Delete the retired observer output macro refusal fixture.
  - `crates/harmos-macros/tests/fixtures/unknown_grant.rs` — Delete removed plugin/observer/catalog macro and grant checks when their production macros are removed; keep guest-required variants until `authoring-without-macros` replaces their consumers.
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
  - `crates/harmos-runtime/tests/plugin/reload.rs` — Delete graph reload ordering, plan preflight and generation guards; retain ordinary reconnect and call-lifetime behaviour on `plugin_grpc`.
  - `crates/harmos-runtime/tests/plugin/retirement.rs` — Replace retained stop, cleanup failure and peer isolation journeys with `plugin_grpc` stories; delete graph-dependent retirement and provider retention mechanics.
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
  - `crates/harmos-runtime/tests/system/cpp/domain.rs` — Delete the old C ABI prototype's nominal operation and stream fixture declarations with its story.
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
  - `crates/harmos-runtime/tests/plugin_grpc/managed_environment.rs` — Replace the removed Windows control-pipe environment story with managed SDK launch-value and inherited Windows-path coverage under the host virtual clock.
  - `sdk/rust/src/bin/host-fixture.rs` — Serve lifecycle and environment fixture operations as SDK builder values; retain cleanup evidence for managed shutdown.
  - `sdk/rust/src/plugin.rs` — Replace native embedded catalog fixture assertions with inert SDK catalog construction and typed section round-trip coverage.
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
  - `examples/schematic/sidecar/Cargo.toml` — Migrate to `harmos` builder values from `sdk/rust` and managed serving; remove framed registration and benchmark-only transport setup.
  - `examples/schematic/sidecar/build.rs` — Migrate to `harmos` builder values from `sdk/rust` and managed serving; remove framed registration and benchmark-only transport setup.
  - `examples/schematic/sidecar/src/benchmark.rs` — Migrate to `harmos` builder values from `sdk/rust` and managed serving; remove framed registration and benchmark-only transport setup.
  - `examples/schematic/sidecar/src/lib.rs` — Migrate to `harmos` builder values from `sdk/rust` and managed serving; remove framed registration and benchmark-only transport setup.
  - `examples/schematic/sidecar/src/main.rs` — Migrate to `harmos` builder values from `sdk/rust` and managed serving; remove framed registration and benchmark-only transport setup.
  - `examples/schematic/sidecar/src/registration.rs` — Migrate to `harmos` builder values from `sdk/rust` and managed serving; remove framed registration and benchmark-only transport setup.
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
- [x] 2.4 Canonicalize file options across SDK fingerprints, share Rust clock descriptors and pin both built-in clock fingerprints in conformance.

- [x] 2.5 Extend conformance and every language fixture for schema graphs, setting-only observers and canonical descriptor metadata.
- [x] 2.6 Consolidate wire definitions in proto/generated protocol types; put only light Rust helpers in protocol and specify cross-language helper semantics in words with matching SDK implementations.

## 3. Host

- [x] 3.1 Add the managed placement: start the process, read the address it announces, dial and supervise it, and call `Stop` before closing.
- [x] 3.2 Add the remote placement: connect to a URL, reconnect, and expose the start time of each activation.
- [x] 3.3 Compute and admit the catalog digest; route child calls on the invoke call.
- [x] 3.4 Open and close `Publish` calls per declared stream, send ticks for interval streams and ingest samples.
- [x] 3.5 Lift the simulation clock into one clock per host with a monotonic and a date reading, and move every time read and wait in the runtime, storage, the client-facing gRPC layer and the built-in operations onto it.
- [x] 3.6 Serve `harmos.clock.sleep` and `harmos.clock.now`, and drive observer ticks from host timers.
- [x] 3.7 Add the virtual mode: its own timer queue, activity accounting for calls, ticks and host work, and a controller that advances while a condition is unmet, with simulation's existing tests kept passing.
- [x] 3.8 Reduce visibility to `Private` and `Public`.
- [x] 3.9 Prove Python conformance plugin admission and host-clock sleep through the real runtime under virtual time.
- [x] 3.10 Before Dokime's Harmos-adoption wave 2, extend client-facing gRPC declarations/codecs to accept generated protobuf types and authoritative descriptors directly; preserve method paths, fingerprints, decoding-memory admission, cancellation, backpressure and all host streaming envelopes.
  - [x] 3.10.1 Register generated message declarations with authoritative descriptors, native endpoints and protobuf fingerprints in domain/operations/runtime.
  - [x] 3.10.2 Prove generated unary and streamed gRPC calls, authorization, reflection and decoding-memory refusal with a vendored-compiler test proto; document the declaration.
  - [x] 3.10.3 Support private generated native operations and typed child calls with registration-time callee dependency checks.

- [x] 3.11 In the host wave, migrate built-in request/results to protocol messages with declaration, gRPC mapping, callee policy and handler in each operation file; register directly through narrow resources without a runtime dependency or per-service adapters.
  - [x] 3.11.1 Migrate lifecycle status, cancel and watch with generated messages and native/gRPC proof.
  - [x] 3.11.2 Migrate clock operations and remove the plugin-specific clock binding.
  - [x] 3.11.3 Migrate host discovery and remove its Rust wire mirrors.
  - [x] 3.11.6 Migrate stream publication, retaining guest-private payload support.
  - [x] 3.11.4 Migrate storage operation families and native streaming consumers.
    - [x] 3.11.4.1 Migrate storage status, file mutation receipts and directory native streams.
    - [x] 3.11.4.2 Migrate file reads, capture and following, preserving grants and terminal stream closure.
    - [x] 3.11.4.3 Migrate schema registration and delete its Rust wire wrappers.
    - [x] 3.11.4.4 Migrate queries, generated native downloads and scoped callers; delete the Rust query-output wire mirror.
    - [x] 3.11.4.5 Migrate record, stream and object following; delete the followed-record Rust wire types.
    - [x] 3.11.4.6 Migrate streamed Arrow/file writes and delete Rust receipt/completion wire models and their Postcard codecs.
    - [x] 3.11.4.7 Migrate hashing and its guest-private callee boundary.
  - [x] 3.11.5 Migrate example operations and remove superseded host projections and wire mirrors.
    - [x] 3.11.5.1 Migrate native example operations, SDK consumers and generated application contracts; isolate deferred guest declarations.
    - [x] 3.11.5.2 Remove replaced native host DTOs and unused storage projections; retain the codec machinery still required by deferred guest/declaration authoring.
- [x] 3.12 Prove generated protobuf unary and streaming declarations through real gRPC routing, bounded decoding and the client authorization hook; provide the adoption example for Dokime's host-wave operations migration.

## 4. Rust SDK

- [x] 4.1 Make `sdk/rust` the public package and library `harmos`, compiling no host code; delete `crates/harmos-sdk` and the `crates/harmos` facade, move host exports to `harmos-runtime` and migrate every consumer.
- [x] 4.1.1 Implement the native gRPC plugin SDK in `sdk/rust` with conformance fixtures.
- [x] 4.2 Integrate the builder surface across native consumers with `serve`; delete plugin, observer and catalog attribute macros with their sidecar variants, retaining macros required for guest or operation/state authoring until `authoring-without-macros`.
- [x] 4.2.1 Add the Rust Message derive and native plugin, operation, stream and observer builder values.
- [x] 4.3 Implement native plugin SDK observers answering host ticks, route its waits through the host clock, and share publish_on and its error type.
- [x] 4.4 Migrate the examples and fixtures.
  - [x] 4.4.1 Migrate Schematic advice to SDK protobuf builders and managed placement, host analysis and transfer streaming, native packaging and benchmark selectors.
  - [x] 4.4.2 Migrate the stream-consumer fixture and its host-row journey to SDK unary calls on managed placement.
  - [x] 4.4.3 Retire or migrate crates/harmos-sidecar/tests/fixtures/catalog_sidecar.rs, crates/harmos-runtime/tests/fixtures/process_client.rs and process_provider.rs, and the native macro fixture consumers under crates/harmos-macros/tests; authoring fixture libraries select `harmos` for SDK builders and `harmos_runtime` for host authoring.
- [x] 4.5 Move surviving native invocation, resource and serving engine code from `crates/harmos-sdk` to host owners with narrow resources for operations; isolate guest dispatch, envelopes and authoring support privately in `harmos-guest` and the runtime guest host, with guest protocol migration deferred under 7.1.
  - [x] 4.5.1 Separate `OperationFactory` installation from its shared `WireTable` callback in `crates/harmos-operations/src/execution/operations/registration.rs`; move guest-only dispatch, codec and lifecycle ownership into private guest modules and rerun guest integration and system stories before native removal.
    - [x] 4.5.1.1 Remove the table-install callback; give guests a private table, lifecycle owner and envelope codec, with a guest-only artifact macro crate; retain erased operation preparation in operations and prove guest integration and system stories.
    - [x] 4.5.1.2 Finish the runtime guest host extraction: private `src/guest/host/envelope/` owns its codec and `src/guest/host/plugin/` owns guest lifecycle, child controls and scopes; native placement shares no guest table or generation scope.
- [x] 4.6 Verify plugin and client SDK dependency closures contain no host crates; migrate example/build-script imports to protocol or runtime as appropriate, and update README/product guidance for the public entry point.

- [x] 4.7 Build the SDK client half over generated application contracts and protocol without host dependencies; add client examples and dependency-closure proof, updating README and product guidance. Task 4.6 proves the existing SDK boundary, not a completed client implementation.
  - [x] 4.7.1 Add typed generated-contract binding, bounded managed unary/upload/download/exchange calls and cancellation settlement in `sdk/rust/src/client*`.
  - [x] 4.7.2 Prove real host interoperability, malformed completion/status handling and the normal/build dependency closure; compile the SDK client example and update README/product guidance.

  - [x] 4.7.3 Expose one independently owned input writer and prove concurrent upload/download progress in the real-host exchange story.

## 5. Pilot SDKs

- [x] 5.1 Python: `sdk/python` over `harmos.v1`, passing the conformance suite.
- [x] 5.2 C#: `sdk/csharp` over `harmos.v1`, passing the conformance suite.
- [x] 5.3 C++: `sdk/cpp` over `harmos.v1`, passing the conformance suite.

## 6. Removal

- [x] 6.1 Delete the framed wire, handshake, credits, control pipe and binary carriers.
- [x] 6.2 Delete publication leases, admission, cadence reports and the native observer runner.
- [x] 6.3 Delete native plugin graph, start ordering and reload plans; grants and provider selection; generation tokens and stale-call fencing. Deferred guest lifecycle/child controls retain private authority and identity checks under 7.1.
- [x] 6.4 Delete `crates/harmos-ffi` and its remaining manifest, task and consumer references.
- [x] 6.4.1 Replace the C ABI bridge under `sdk/cpp` with the gRPC SDK.
- [x] 6.5 Migrate public host operation payloads and shared host/guest assignment catalogs to protobuf-derived definitions and remove Postcard from the host wire contract. Built-ins and native examples use authoritative generated messages; projection machinery still used by deferred guest/declaration authoring remains until those consumers migrate. The native plugin derive is covered by 4.2.1; the user-deferred guest-owned private command, lifecycle, child and stream payload codecs remain with 7.1 as recorded in the design.
  - [x] 6.5.1 Migrate public host encoded requests/results and operation contract identities to protobuf; retain private guest matching codecs and stored-value formats.
  - [x] 6.5.2 Replace shared assignment catalog Postcard encoding with protocol messages and validate catalog freezing and malformed input.
  - [x] 6.5.3 Document the precise guest-owned private Postcard codecs deferred to 7.1.

- [x] 6.6 Migrate the runtime sidecar behaviour tests and delete the transport tests and benches.
  - [x] 6.6.1 Complete targeted proof of the replacement Schematic composition and host-row consumption journeys.
  - [x] 6.6.2 Migrate the remaining retained behaviour under crates/harmos-runtime/tests/{sidecar,observers,operations/wire,application,plugin,runtime,integration} and remove assertions for retired graph/grant/token/lease/cadence mechanisms.
    - [x] 6.6.2.1 Retire the process graph and graph reload-plan stories, the framed plugin upload/download/exchange transport stories and their fixture registrations, and the C ABI prototype story, feature and guide.
  - [x] 6.6.3 Delete framed transport tests and benches in crates/harmos-sidecar and native transport benchmark branches in crates/harmos-runtime/benches, preserving guest-used support.
- [x] 6.7 Update the product guide, README and authoring guidance.
- [x] 6.8 After authoritative host protobuf adoption (2.6, 3.10–3.12 and host 6.5), `authoring-without-macros` in its agreed order and derive consolidation (6.10), reduce domain to the native foundation in design.md, Crate Structure; delete replaced wire mirrors and dead conversions.
  - [x] 6.8.0 Migrate operation declarations/policies, listener and stream authoring, retained guest inventories, all Rust consumers and native-foundation docs; retain stored schema/recovery/query behavior and leave guest protocol consolidation under 7.1.
  - [x] 6.8.1 Retain `schema.rs`, `schema/`, `message/` stored encoding, canonical revisions, validation and allocation accounting; retain `identity.rs`, native `metadata*`, `state/` and `journal.rs` identities, preparation, restoration, inverse pairing and positions.
  - [x] 6.8.2 Retain `model/storage/`, including query selections/transforms, schema/object catalogs, revisions, history identity, mutation receipts and files; replace transport representations with protocol types without changing persisted semantics.
  - [x] 6.8.3 Split `operations/`, `model/operations/` and `model/services/`: metadata/visibility/transport descriptions to protocol; dispatch, call context, streaming and policy validation to operations with narrow runtime-supplied resources.
  - [x] 6.8.4 Split `listeners/`, `observers.rs` and `streams/`: portable declarations to protocol, listener activation and host behaviour to runtime, persisted row identity retained natively.
  - [x] 6.8.5 Delete legacy `inventory*`, `plugin/`, `artifacts.rs` and `registration.rs` inventories, grants and markers as replacements land; retain guest-required mechanisms only until 7.1. Split the remaining modules enumerated in Crate Structure by owner rather than retain mixed modules wholesale.
  - [x] 6.8.6 Migrate manifests/re-exports and all consumers, including lightweight application models; preserve the downward runtime → operations → storage → domain → protocol graph and SDK → protocol boundary. Account for focused stored-schema, recovery/inverse and storage-query proof and affected README/topic guidance.
- [x] 6.9 Redirect contract-only operations consumers to protocol/SDK, then remove the operations `runtime` feature without moving handlers out of their operation files. The superseded host SDK root and facade are already removed under 4.1/4.5.
- [x] 6.10 After the ordered `authoring-without-macros` replacements, merge `harmos-macros` into `sdk/rust/derive` as the single derive crate (`harmos-sdk-derive`), retaining only field-reading derives with distinct stored-schema, protobuf Message and row semantics. Coordinate that change's 4.3; migrate dependencies, re-exports, consumers and coverage, then delete the old package without adding host dependencies to the SDK.
- [x] 6.11 Move `harmos-conformance` to the tool package `tools/conformance` at any time, outside the six-library core; preserve binary names, CLI/release interfaces, all 19 protocol cases, managed/remote driving, reporting and Rust test integration. Keep its protocol-only product dependency; migrate SDK launchers, fixtures, tasks, docs, manifests and planned archive paths with `release-assets-layout`.
- [x] 6.12 Split `harmos-packaging` at any time: move build, watch/cancellation, componentization, inspection, bundle construction and proto/Python generation into `tools/packaging`; retain only required bundle reading in runtime. Remove normal runtime tooling dependencies and migrate build scripts, manifests, tasks, launchers, fixtures and README/topic guidance together.

## 7. Guests

- [x] 7.1 After domain reduction, complete guest lifecycle/child-control design and migrate guest command/child envelopes to `harmos.v1`, deleting the Postcard envelope codec together with the guest SDK consolidation below; native cutover does not establish guest parity.
  - [x] 7.1.1 Specify replacements for lifecycle phases and child submit, status, join and release controls, preserving required behaviour and authority before removing private guest mechanisms.
  - [x] 7.1.2 Fold `harmos-guest` into optional SDK WebAssembly support; replace `harmos-guest-macros`' `plugin`/`harmos_guest::guest` declarations with builder values and one author-supplied registration function, then delete both old packages.
  - [x] 7.1.3 Keep generated component bindings, canonical ABI exports/resources, async lifting and WIT register/operate glue inside the SDK adapter; keep guest loading, fuel/memory limits and sandbox authority in runtime without SDK host dependencies.
  - [x] 7.1.4 Move sandbox fixtures and guest tests with the adapter; migrate example/host consumers, manifests, build/generation tooling, docs and re-exports. Remove replaced guest inventories/grants/markers and account for focused lifecycle, child-control, recovery and sandbox proof on Linux and Windows.

## 8. Final Validation

- [x] 8.6 Replace the SDK call style across the four languages in `refactor/sdk-call-style`; one writer owns `sdk/**`, client/plugin generators in `tools/**`, example client/plugin code and SDK guides. Storage, query and runtime storage stay with their separate owner. Retain local commits only.
  - [x] 8.6.1 Add descriptor-derived required arguments, optional options, retry identity, owned context and payload projection in generators; mark optional selectors in `proto/**` and regenerate outputs through tasks.
  - [x] 8.6.2 Replace public wire/version paths with root, client and plugin barrels; migrate authoring, examples, fixtures, composition consumers and guides; remove superseded request-object overloads.
  - [x] 8.6.3 Apply import-at-top and one-statement-per-line conventions and minimal commented formatter deviations; account for README and architecture impact.
  - [x] 8.6.4 Prove focused calls, options, retry identity, streaming projection and library/product composition before source closure.
  - [x] 8.6.5 On the committed candidate run format, all language/repository checks, each SDK build/test, conformance (36 rules each), example and packaged consumer stories, and generation idempotence; report exact failures and counts. Windows proof passes on the committed source: Rust 20 SDK tests plus one WebAssembly system story, Python 8, C# 13, C++ 12 units plus 2 conformance stories; every SDK fixture and packaged consumer passes all 36 rules. All language/repository checks and builds pass, along with 68 tooling, 4 product-binding and 2 touched runtime stories. All four generation tasks reproduce a clean tree. Native Linux proof remains under 8.1 and 8.2.

- [x] 8.5 Complete the additive SDK follow-ups in this worktree; ownership is SDK client, registration and fixtures, tools/conformance, runtime plugin dispatch and its stories, and this change's design/tasks. Storage and storage-facing contracts remain with their existing owners. README impact is reviewed with the public API notes.
  - [x] 8.5.1 Add descriptor-backed generated-client decoded-memory admission and focused real-host refusal, round-trip and cancellation proof.
    - [x] 8.5.1.1 Require caller-owned `ClientSettings.retained_bytes`, scan repeated/map/nested values before decoding, merge borrowed managed envelopes, retain claims in SDK-owned queues and release at handoff without changing public call shapes.
    - [x] 8.5.1.2 Prove compact-message refusal before decoding, repeated/map/nested accounting, queued claims, scratch release and decode-error cleanup in five SDK unit tests; retain the malformed-completion/status gRPC journey.
    - [x] 8.5.1.3 Run the expanded real-host client journey covering unary/discovery/item/final refusal, round trips, slow consumption, shared claims, cancellation and final-response handoff. The integrated RecordStore supports recovery collection; the expanded journey passes on Windows, alongside all five admission unit guards and the malformed-completion/status journey.
  - [x] 8.5.2 Carry caller-owned invocation scope through managed/remote plugin dispatch and inherited child calls; prove scope delivery and unscoped refusal. Windows focused proof passes: plugin_grpc::scope::bound_scope_reaches_handlers_and_children_and_missing_scope_is_refused, including two isolated scopes, native and plugin children, both placements, refusal and cleanup. The default-feature runtime package check also passes; final gates remain separate under 8.5.4.
  - [x] 8.5.3 Add independent message registration to all four SDKs and a shared descriptor-only conformance case. All 20 shared cases pass for Rust, Python, C# and C++ on Windows; C# also proves remote placement. Rust descriptor merging now retains top-level enums when independent messages are registered first.
  - [x] 8.5.4 Complete the requested validation and documentation follow-ups without treating source closure as proof.
    - [x] 8.5.4.1 Run Windows format, the complete Rust and repository checks, the test-artifact build and all four SDK conformance journeys. SDK-specific Python formatting, lint, typing and generated-binding drift also pass. Merge proof additionally passes all 120 storage integration stories including the benchmark boundary, the generated storage-consumer journey, named client/scope journeys and five admission unit guards. C++ passes all 20 cases using a temporary H: drive alias for MSVC path limits; recreate that alias to reuse the current CMake cache. Native Linux proof remains under 8.1 and 8.2.
    - [x] 8.5.4.2 Complete the whole Python check against generated Product clients and the current focused conformance host; formatting, lint, typing, tooling/SDK journeys and generated-client checks pass on Windows. Merge proof selects the freshly built target/debug/harmos-conformance.exe with HARMOS_CONFORMANCE_HOST; the older Product export remains stale.
    - [x] 8.5.4.3 Update the current reference-case count in docs/tooling.md to 20; document required generated-client retained_bytes, configuration and accounting ending at handoff; update sdk/rust/examples/client.rs.

  - [x] 8.5.5 Replace the invocation scope vocabulary across runtime, four SDKs, scoped story, fixtures and guides; use Scope, with_scope and author-required scope accessors without aliases or operation-name policy.
  - [x] 8.5.6 Prove the renamed scoped story, four SDK conformance suites (20 cases each), format, Rust/Python/repository checks, Tests artifact build and retired-name search; commit locally without pushing. All requested Windows proofs pass through focused tasks. The optional archive consumer still requires separate Product inspections; its preflight stopped before tests because schematic-sidecar was absent.

- [ ] 8.1 Run targeted transport, lifecycle, conformance and guest tests on Linux and Windows.

- [ ] 8.2 Run each pilot SDK against the extended conformance suite on native Linux and Windows; retain failures and repair affected consumers.

- [x] 8.3 For the generated host contract and wire cutover, run format, Rust/Python/repository checks and the test artifact build on Windows; keep native Linux proof under 8.1.
- [x] 8.4 Complete the requested Windows format, Rust/Python/repository checks, test-artifact build and four-SDK conformance after the domain reduction and generated Rust client; native Linux proof remains under 8.1.

## Workflow Follow-Up

- Dokime adopts the merged revision in its own `grpc-plugins` change.
- Dokime wave 2 adopts 3.10; host wave 3 moves server behaviour to operations with direct registration and deletes each area's `hosted!`/`Host`/`Serve`; waves 4–5 redirect CLI/desktop hosting to runtime and delete server/domain crates and dead conversions as specified in Crate Structure.
- After the last gRPC wave, Harmos `storage-interface` implements and benchmarks the generic extensions; Dokime's change of the same name consumes them, extracts application persistence and supplies before/after multi-day benchmarks. No storage extension or extraction runs in a gRPC wave.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
- SDK system tests run in Nightly only, not on pull requests.

## Repository Layout and Nightly Follow-Up

- [x] Move explicit git-cliff/Taplo configuration to `tools/config/`, remove unused
  Typos configuration and its ownership records, and move contributor guidance to
  `docs/CONTRIBUTING.md`; update callers, editors, templates, snippets and links.
- [x] Place Ruff caches under `tools/python/` and document tool configuration owners.
- [x] Restrict dev pushes to check/build jobs and harmonize Nightly freshness,
  cancellation, task/job names and run-owned reporting with the sibling product.
- [x] Run the requested format, repository and Python checks and repair owned findings.
- [ ] Complete generated-client proof after the combined Product producer supplies
  `target/generated/python/src` and
  `target/build/dev/windows-x86_64/product/examples/catalog.json`. The Python check
  passed formatting, lint, protocol drift, 45 tooling tests and both SDK journeys,
  then failed on the absent generated-client directory. Focused conformance build,
  format and repository checks passed; Linux and hosted proof remain open.
