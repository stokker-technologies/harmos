// SPDX-License-Identifier: Apache-2.0

using System.Collections.Generic;
using System.Threading.Tasks;
using System.Threading;
using System;
using Google.Protobuf;
using Grpc.Core;
using Harmos.Clients;
using Harmos.Plugins;
using Harmos;
using Xunit;
using FixtureClient = Fixture.Client;
using Status = Grpc.Core.Status;

namespace Harmos.Tests.Unit;

public sealed class ResourceRoots
{
  [Fact]
  public void CallerSurfacesExposeSharedOperationsAndKeepAdministrationOnClients()
  {
    Assert.NotNull(typeof(InvocationsResource).GetMethod("Create"));
    Assert.Null(typeof(HostInvocationsResource).GetMethod("Create"));
    Assert.NotNull(typeof(PluginsSettingsResource).GetMethod("Set"));
    Assert.Null(typeof(HostPluginsSettingsResource).GetMethod("Set"));
    var clientSettings = typeof(PluginsSettingsResource).GetMethod("List")!;
    var pluginSettings = typeof(HostPluginsSettingsResource).GetMethod("List")!;
    Assert.Equal(clientSettings.ReturnType, pluginSettings.ReturnType);
    Assert.Equal(typeof(Task<SettingsListResponse>), pluginSettings.ReturnType);
    Assert.Null(typeof(HostRoot).GetProperty("Access"));
    Assert.Null(typeof(HostRoot).GetProperty("Status"));
    Assert.NotNull(typeof(HostTimeResource).GetMethod("Get"));
    Assert.NotNull(typeof(TimeResource).GetMethod("Get"));
  }

  [Fact]
  public async Task DynamicInvocationRoutesCreateWithTheDiscoveredContractAndPayload()
  {
    var client = new Client(new Invoker());
    var payload = ByteString.CopyFromUtf8("input");
    var contract = (await client.Operations.Get("fixture.echo")).Contract;
    var response = await client.Invocations.Create(contract.Id, contract.Fingerprint, payload);
    Assert.Equal(payload, response);
  }

  [Fact]
  public async Task LibraryAndProductServicesShareParentsAndRetainWatchCardinality()
  {
    var invoker = new Invoker();
    var client = new FixtureClient(invoker);
    var status = await client.Status.Get();
    Assert.Equal("harmos.host.v1.Status/Get:selection", status.Status.Identity.ApplicationName);
    var diagnostics = await client.Status.Diagnostics.Get("selection", 0, 512);
    Assert.Equal("harmos.host.v1.status.Diagnostics/Get:selection", diagnostics.Name);
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
        Assert.Equal("/harmos.host.v1.Operations/Get", method.FullName);
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
        Assert.Equal("/harmos.host.v1.Invocations/Create", method.FullName);
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
        new(Task.FromResult(Reply(method, request)), Task.FromResult(new Metadata()), () => Status.DefaultSuccess, () => new Metadata(), () => { });
    public override AsyncServerStreamingCall<TResponse> AsyncServerStreamingCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options, TRequest request) =>
        new(new Reader<TResponse>([Reply(method, request, ":opened"), Reply(method, request, ":finished")]), Task.FromResult(new Metadata()), () => Status.DefaultSuccess, () => new Metadata(), () => { });
    public override TResponse BlockingUnaryCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options, TRequest request) => throw new NotSupportedException();
    public override AsyncClientStreamingCall<TRequest, TResponse> AsyncClientStreamingCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options) => throw new NotSupportedException();
    public override AsyncDuplexStreamingCall<TRequest, TResponse> AsyncDuplexStreamingCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options) => throw new NotSupportedException();
  }
}
