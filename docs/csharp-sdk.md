# C# SDK

The `Harmos` NuGet package targets .NET 10. Import `Client`, shared values and
errors from `Harmos`, client responses from `Harmos.Clients`, and plugin authoring
types from `Harmos.Plugins`. Wire requests, stubs and plugin carriers are internal.

## Client Calls

Calls take request content as arguments. Each call owns its transport state and
accepts optional `Grpc.Core.CallOptions` for metadata, cancellation and an explicit
deadline override. Responses with one field return that field; multi-field
responses retain their complete response type. Watches yield projected payloads
and cancel when their enumerator is disposed.

```csharp
using Harmos;

using var client = Client.Connect("http://127.0.0.1:50051");
var operations = await client.Operations.List();
var settings = await client.Plugins.Settings.List(name: "example");
```

The [client example](../sdk/csharp/examples/Client/Program.cs) lists library
operation contracts. Its separate `--invoke <operation> <base64-payload>` branch
discovers a contract, then calls `client.Invocations.Create` with its identity,
fingerprint and encoded payload. Typed application operations belong in an
application's generated client.

## Plugin Settings and Context

The [plugin example](../sdk/csharp/examples/Plugin/Program.cs) serves an echo
operation and reads its own library-stored prefix setting. Declare defaults and
requirements when building the plugin:

```csharp
using Harmos;
using Harmos.Plugins;

var plugin = new Plugin("example", "1.0.0")
    .Setting("prefix", new SettingValue { Text = "" })
    .Requires(HostOperation.PluginsSettingsList);
```

Inside a handler, `await context.Host.Plugins.Settings.List()` reads the caller's
entry. A plugin may also pass its own entry name; the host refuses another name.
The response includes declarations, complete values, revision and `ResolvedAt`.
Clients supply an entry name. `client.Plugins.Settings.Set` replaces the complete
value map and is available only to clients.

The generator reads each operation's callers option: shared operations appear on
both the client and plugin context; client administration stays on the client.
`Time`, `Operations` and invocation Get/Cancel/Watch are shared. Invocation Create
is client-only. A product's declarative composition may narrow these audiences;
see [Product Composition and Narrowing](architecture.md#product-composition-and-narrowing).
The host remains responsible for caller authorization.

An observer can call declared host operations and watches while its tick is
unanswered. Child IDs remain unique across ticks on one Samples call; independent
calls retain independent scopes. Answering a tick cancels outstanding children;
ending demand cancels the active observer context. Device streams cannot issue
child calls. Timed waits use the host clock and operation policies.

## Build and Validation

Install the pinned SDK with `mise -C sdk/csharp install --locked`. `Harmos.slnx`
contains the library, tests, conformance fixture and runnable examples.

```sh
mise -C sdk/csharp run build
mise -C sdk/csharp run test -- --layer unit
mise -C sdk/csharp run test -- --filter FullyQualifiedName~ClientCallContext
mise -C sdk/csharp run check
mise -C sdk/csharp run package
```

Test and package consume existing build outputs. `--profile release` selects
Release. `--layer integration` includes the managed/remote reference-host journey;
build `harmos-conformance` first using its package selection. `--filter` selects
individual journeys and takes precedence over `--layer`. Direct carrier tests
exercise settings, clock, correlation, failures, watch demand and cancellation
without a Rust host. Native Linux and Windows validation remain separate proofs.

`mise -C sdk/csharp run test:tooling -- --exact tools.python.tests.integration.test_csharp_tooling`
checks C# generation, caller narrowing and fixture staging. Generated roots and
wire bindings are committed; `mise -C sdk/csharp run generate` regenerates them,
and `check` compares output without rewriting sources.

`Directory.Build.props` owns framework, nullable checking, warnings as errors,
analysis and package metadata. `Directory.Packages.props` owns dependency versions;
restores use the committed lock files. Intentional dependency updates use
`dotnet restore Harmos.slnx --force-evaluate -p:RestoreLockedMode=false` inside the
SDK tool environment, followed by committing the updated locks.

All build, intermediate, test and package output lives under
`sdk/csharp/artifacts/`. `Directory.Build.targets` records each executable path in
`artifacts/paths/<Configuration>/<Project>.txt`; fixture consumers read those
manifests. Package versions come from the workspace `Cargo.toml`; package output
contains `Harmos.<version>.nupkg` and its symbol package. Tasks do not publish.
The SDK supplies SourceLink; CI enables deterministic integration builds.

A package consumer references `Harmos`, targets .NET 10 and retains its own
application protobuf generation. The test-owned conformance fixture compiles
against the package separately; its messages and implementation are not shipped
in the library. Products declare their own wire operations over native storage;
`Transfer.MessageBytes` retains shared carrier sizing for those wrappers.
