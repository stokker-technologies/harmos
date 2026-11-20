// SPDX-License-Identifier: Apache-2.0

using System.Collections.Generic;
using System.Linq;
using System.Threading.Channels;
using Harmos.Internal.Plugin;
using System.Threading.Tasks;
using System.Threading;
using System;
using Google.Protobuf;
using Grpc.Core;
using Harmos.Clients;
using Harmos.Plugins;
using Harmos;
using Xunit;

namespace Harmos.Tests.Unit;

public sealed class ResourceRoots
{
  [Fact]
  public async Task InitializeContextStatePreservesBytesAndRevision()
  {
    var declarations = new Harmos.Plugins.Plugin("example", "1.0.0")
        .Register(ctx => ctx.Register(HostOperation.PluginsStateGet))
        .Register(ctx => ctx.Register(HostOperation.PluginsStatePut)).Declarations();
    var frames = Channel.CreateUnbounded<InstanceInvokeResponse>();
    using var cancellation = new CancellationTokenSource();
    var context = new Harmos.Plugins.Context(new Request { Operation = "example.initialize" }, "example",
        declarations.Requires.ToDictionary(requirement => requirement.Id, requirement => requirement.Fingerprint), cancellation, frames.Writer);
    var getting = context.State.Get();
    var child = (await frames.Reader.ReadAsync()).Child;
    Assert.Equal("harmos.plugin.v1.plugins.State/Get", child.Operation);
    await context.Reply(new ChildResponse { Call = child.Call, Payload = new StateGetResponse { Value = ByteString.CopyFromUtf8("old"), Revision = 3 }.ToByteString() });
    var saved = await getting;
    Assert.Equal(ByteString.CopyFromUtf8("old"), saved.Value);
    var putting = context.State.Put(saved.Revision, ByteString.CopyFromUtf8("intent"));
    child = (await frames.Reader.ReadAsync()).Child;
    var request = StatePutRequest.Parser.ParseFrom(child.Payload);
    Assert.Equal(3UL, request.ExpectedRevision);
    Assert.Equal(ByteString.CopyFromUtf8("intent"), request.Value);
    await context.Reply(new ChildResponse { Call = child.Call, Payload = new StatePutResponse { Revision = 4 }.ToByteString() });
    Assert.Equal(4UL, await putting);
  }

  [Fact]
  public void CallerSurfacesExposeSharedOperationsAndKeepAdministrationOnClients()
  {
    Assert.NotNull(typeof(Fixture.InvocationsResource).GetMethod("Create"));
    Assert.Null(typeof(HostInvocationsResource).GetMethod("Create"));
    Assert.NotNull(typeof(Fixture.PluginsSettingsResource).GetMethod("Set"));
    Assert.Null(typeof(HostPluginsSettingsResource).GetMethod("Set"));
    var clientSettings = typeof(Fixture.PluginsSettingsResource).GetMethod("List")!;
    var pluginSettings = typeof(HostPluginsSettingsResource).GetMethod("List")!;
    Assert.Equal(clientSettings.ReturnType, pluginSettings.ReturnType);
    Assert.Equal(typeof(Task<SettingsListResponse>), pluginSettings.ReturnType);
    Assert.Null(typeof(HostRoot).GetProperty("Access"));
    Assert.Null(typeof(HostRoot).GetProperty("Status"));
    Assert.NotNull(typeof(HostTimeResource).GetMethod("Get"));
    Assert.NotNull(typeof(Fixture.TimeResource).GetMethod("Get"));
    Assert.NotNull(typeof(Fixture.Plugins.HostTimeResource).GetMethod("Get"));
    Assert.NotNull(typeof(Fixture.Plugins.HostTimeResource).GetMethod("Sleep"));
    Assert.NotNull(typeof(Fixture.TimeResource).GetMethod("Sleep"));
  }

  [Fact]
  public async Task DynamicInvocationRoutesCreateWithTheDiscoveredContractAndPayload()
  {
    var client = new Fixture.Client(new Invoker());
    var payload = ByteString.CopyFromUtf8("input");
    var contract = (await client.Operations.Get("fixture.echo")).Contract;
    var response = await client.Invocations.Create(contract.Id, contract.Fingerprint, payload);
    Assert.Equal(payload, response);
  }

  [Fact]
  public async Task LibraryAndProductServicesShareParentsAndRetainWatchCardinality()
  {
    var invoker = new Invoker();
    var client = new Fixture.Client(invoker);
    var status = await client.Status.Get();
    Assert.Equal("application.host.v1.Status/Get:selection", status.Status.Identity.ApplicationName);
    var diagnostics = await client.Status.Diagnostics.Get("selection", 0, 512);
    Assert.Equal("application.host.v1.status.Diagnostics/Get:selection", diagnostics.Name);
    var started = await client.Jobs.Start("selection");
    Assert.Equal("application.host.v1.Jobs/Start:selection", started);
    var progress = new List<string>();
    await foreach (var item in client.Jobs.Progress.Watch("selection"))
    {
      progress.Add(item);
    }
    Assert.Equal(["application.host.v1.jobs.Progress/Watch:selection:opened", "application.host.v1.jobs.Progress/Watch:selection:finished"], progress);
    var annotation = await client.Plugins.Annotations.Get("example");
    Assert.Equal("application.host.v1.plugins.Annotations/Get:example", annotation);
    var values = new SettingValues();
    var generated = await client.Plugins.Settings.Set("example", values, ByteString.Empty);
    Assert.False(generated.Revision.IsEmpty);
    var replayed = await client.Plugins.Settings.Set("example", values, ByteString.Empty, requestId: "replay");
    Assert.Equal("replay", replayed.Revision.ToStringUtf8());

  }

  private sealed class Reader<T>(IEnumerable<T> values) : IAsyncStreamReader<T>
  {
    private readonly Queue<T> remaining = new(values);
    public T Current { get; private set; } = default!;
    public Task<bool> MoveNext(CancellationToken cancellationToken)
    {
      cancellationToken.ThrowIfCancellationRequested();
      var ready = remaining.TryDequeue(out var value);
      if (ready)
        Current = value!;
      return Task.FromResult(ready);
    }
  }

  private sealed class Invoker : CallInvoker
  {
    private static TResponse Reply<TRequest, TResponse>(Method<TRequest, TResponse> method, TRequest request, string suffix = "")
      where TRequest : class where TResponse : class
    {
      if (request is OperationsGetRequest discovery)
      {
        Assert.Equal("/application.host.v1.Operations/Get", method.FullName);
        return (TResponse)(object)new OperationsGetResponse
        {
          Operation = new DiscoveredOperation
          {
            Contract = new DiscoveredContract { Id = discovery.Id, Fingerprint = ByteString.CopyFromUtf8("contract") }
          }
        };
      }
      if (request is InvocationsCreateRequest invocation)
      {
        Assert.Equal("/application.host.v1.Invocations/Create", method.FullName);
        Assert.Equal("fixture.echo", invocation.Operation);
        Assert.Equal(ByteString.CopyFromUtf8("contract"), invocation.Fingerprint);
        return (TResponse)(object)new InvocationsCreateResponse { Payload = invocation.Payload };
      }
      if (request is SettingsSetRequest settings)
      {
        return (TResponse)(object)new SettingsSetResponse { Revision = ByteString.CopyFromUtf8(settings.RequestId) };
      }
      if (request is StatusGetRequest)
        return (TResponse)(object)new StatusGetResponse { Status = new() { Identity = new() { ApplicationName = method.FullName.TrimStart('/') + ":selection" } } };
      var input = (IMessage)(object)request;
      var response = (IMessage)Activator.CreateInstance(typeof(TResponse))!;
      var value = input.Descriptor.FindFieldByNumber(1).Accessor.GetValue(input);
      response.Descriptor.FindFieldByNumber(1).Accessor.SetValue(response, method.FullName.TrimStart('/') + ":" + value + suffix);
      return (TResponse)response;
    }
    public override AsyncUnaryCall<TResponse> AsyncUnaryCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options, TRequest request) =>
        new(Task.FromResult(Reply(method, request)), Task.FromResult(new Metadata()), () => global::Grpc.Core.Status.DefaultSuccess, () => new Metadata(), () => { });
    public override AsyncServerStreamingCall<TResponse> AsyncServerStreamingCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options, TRequest request) =>
        new(new Reader<TResponse>([Reply(method, request, ":opened"), Reply(method, request, ":finished")]), Task.FromResult(new Metadata()), () => global::Grpc.Core.Status.DefaultSuccess, () => new Metadata(), () => { });
    public override TResponse BlockingUnaryCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options, TRequest request) => throw new NotSupportedException();
    public override AsyncClientStreamingCall<TRequest, TResponse> AsyncClientStreamingCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options) => throw new NotSupportedException();
    public override AsyncDuplexStreamingCall<TRequest, TResponse> AsyncDuplexStreamingCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options) => throw new NotSupportedException();
  }
}
