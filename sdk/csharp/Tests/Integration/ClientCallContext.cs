// SPDX-License-Identifier: Apache-2.0

using Google.Protobuf;
using Grpc.Core;
using Xunit;

namespace Harmos.Sdk.Tests.Integration;

public sealed class ClientCallContext
{
  [Fact]
  public async Task OneCallPreservesMetadataDeadlineOverrideAndCancellation()
  {
    // Bind All Three Caller Controls to One Generated Watch.
    var transport = new Invoker();
    var client = new global::Fixture.Sdk.Client(transport);
    using var cancellation = new CancellationTokenSource();
    var deadline = new DateTime(2031, 1, 1, 0, 0, 0, DateTimeKind.Utc);
    var options = new CallOptions(new Metadata { { "authorization", "Bearer fixture" } }, deadline, cancellation.Token);
    using var call = client.Jobs.Progress.Watch(new Application.V1.Jobs.ProgressWatchRequest(), options);
    Assert.True(await call.ResponseStream.MoveNext(CancellationToken.None));
    Assert.Equal("Bearer fixture", call.ResponseStream.Current.Value);
    Assert.Equal(deadline, transport.Options.Deadline);
    // Await Cancellation Without Sampling Time or Waiting for the Deadline.
    var pending = call.ResponseStream.MoveNext(CancellationToken.None);
    cancellation.Cancel();
    await Assert.ThrowsAnyAsync<OperationCanceledException>(async () => await pending);
    await transport.Cancelled.Task;
    // The Following Resource Call Keeps Its Own Default Options.
    using var following = client.Plugins.Get(new Library.V1.PluginsGetRequest());
    await following.ResponseAsync;
    Assert.Null(transport.Options.Deadline);
    Assert.Null(transport.Options.Headers);
    Assert.False(transport.Options.CancellationToken.CanBeCanceled);
  }

  private sealed class Reader<T>(CallOptions options, TaskCompletionSource cancelled) : IAsyncStreamReader<T> where T : class
  {
    private bool opened;
    public T Current { get; private set; } = default!;
    public Task<bool> MoveNext(CancellationToken cancellationToken)
    {
      if (!opened)
      {
        opened = true;
        var message = (IMessage)Activator.CreateInstance(typeof(T))!;
        message.Descriptor.FindFieldByNumber(1).Accessor.SetValue(message, options.Headers?.GetValue("authorization") ?? "");
        Current = (T)message;
        return Task.FromResult(true);
      }
      var pending = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
      options.CancellationToken.Register(() => { pending.TrySetCanceled(options.CancellationToken); cancelled.TrySetResult(); });
      return pending.Task;
    }
  }

  private sealed class Invoker : CallInvoker
  {
    public CallOptions Options { get; private set; }
    public TaskCompletionSource Cancelled { get; } = new(TaskCreationOptions.RunContinuationsAsynchronously);
    public override AsyncServerStreamingCall<TResponse> AsyncServerStreamingCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options, TRequest request)
    {
      Options = options;
      return new(new Reader<TResponse>(options, Cancelled), Task.FromResult(new Metadata()), () => Status.DefaultSuccess, () => new Metadata(), () => { });
    }
    public override AsyncUnaryCall<TResponse> AsyncUnaryCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options, TRequest request)
    {
      Options = options;
      return new(Task.FromResult(Activator.CreateInstance<TResponse>()), Task.FromResult(new Metadata()), () => Status.DefaultSuccess, () => new Metadata(), () => { });
    }
    public override TResponse BlockingUnaryCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options, TRequest request) => throw new NotSupportedException();
    public override AsyncClientStreamingCall<TRequest, TResponse> AsyncClientStreamingCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options) => throw new NotSupportedException();
    public override AsyncDuplexStreamingCall<TRequest, TResponse> AsyncDuplexStreamingCall<TRequest, TResponse>(Method<TRequest, TResponse> method, string? host, CallOptions options) => throw new NotSupportedException();
  }
}
