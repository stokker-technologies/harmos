// SPDX-License-Identifier: Apache-2.0

using System.Collections.Generic;
using System.Threading.Tasks;
using System.Threading;
using System;
using Google.Protobuf;
using Grpc.Core;
using Harmos.Clients;
using Harmos;
using Xunit;
using FixtureClient = Fixture.Client;
using Status = Grpc.Core.Status;

namespace Harmos.Tests.Unit;

public sealed class ResourceRoots
{
  [Fact]
  public async Task LibraryAndProductServicesShareParentsAndRetainWatchCardinality()
  {
    var invoker = new Invoker();
    var client = new FixtureClient(invoker);
    var status = await client.Status.Get();
    Assert.Equal("harmos.client.v1.Status/Get:selection", status.Status.Identity.ApplicationName);
    var diagnostics = await client.Status.Diagnostics.Get("selection", 0, 512);
    Assert.Equal("harmos.client.v1.status.Diagnostics/Get:selection", diagnostics.Name);
    var started = await client.Jobs.Start("selection");
    Assert.Equal("application.v1.Jobs/Start:selection", started);
    var progress = new List<string>();
    await foreach (var item in client.Jobs.Progress.Watch("selection"))
    {
      progress.Add(item);
    }
    Assert.Equal(["application.v1.jobs.Progress/Watch:selection:opened", "application.v1.jobs.Progress/Watch:selection:finished"], progress);
    var annotation = await client.Plugins.Annotations.Get("example");
    Assert.Equal("application.v1.plugins.Annotations/Get:example", annotation);
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
