// SPDX-License-Identifier: Apache-2.0

using Google.Protobuf;
using Grpc.Core;
using Xunit;

namespace Harmos.Tests;

public sealed class ResourceRoots
{
  [Fact]
  public async Task LibraryAndProductServicesShareParentsAndRetainWatchCardinality()
  {
    var invoker = new Invoker();
    var client = new Fixture.Sdk.Client(invoker);
    using var plugins = client.Plugins.Get(new Library.V1.PluginsGetRequest { Value = "selection" });
    Assert.Equal("library.v1.Plugins/Get:selection", (await plugins.ResponseAsync).Value);
    using var settings = client.Plugins.Settings.Get(new Library.V1.Plugins.SettingsGetRequest { Value = "selection" });
    Assert.Equal("library.v1.plugins.Settings/Get:selection", (await settings.ResponseAsync).Value);
    using var started = client.Jobs.Start(new Application.V1.JobsStartRequest { Value = "selection" });
    Assert.Equal("application.v1.Jobs/Start:selection", (await started.ResponseAsync).Value);
    using var progress = client.Jobs.Progress.Watch(new Application.V1.Jobs.ProgressWatchRequest { Value = "selection" });
    Assert.True(await progress.ResponseStream.MoveNext(CancellationToken.None));
    Assert.Equal("application.v1.jobs.Progress/Watch:selection:opened", progress.ResponseStream.Current.Value);
    Assert.True(await progress.ResponseStream.MoveNext(CancellationToken.None));
    Assert.Equal("application.v1.jobs.Progress/Watch:selection:finished", progress.ResponseStream.Current.Value);
    Assert.False(await progress.ResponseStream.MoveNext(CancellationToken.None));
  }

  private sealed class Reader<T>(IEnumerable<T> values) : IAsyncStreamReader<T>
  {
    private readonly Queue<T> remaining = new(values);
    public T Current { get; private set; } = default!;
    public Task<bool> MoveNext(CancellationToken cancellationToken)
    {
      cancellationToken.ThrowIfCancellationRequested();
      var ready = remaining.TryDequeue(out var value);
      if (ready) Current = value!;
      return Task.FromResult(ready);
    }
  }

  private sealed class Invoker : CallInvoker
  {
    private static TResponse Reply<TRequest, TResponse>(Method<TRequest, TResponse> method, TRequest request, string suffix = "")
      where TRequest : class where TResponse : class
    {
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
