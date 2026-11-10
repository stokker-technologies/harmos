// SPDX-License-Identifier: Apache-2.0

using System.Threading.Channels;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Harmos.V1;
using Time = Harmos.V1.Time;

namespace Harmos.Sdk;

/// <summary>One invocation or publication demand. All timed waits use the host clock.</summary>
public sealed class Context
{
  private readonly Dictionary<ulong, TaskCompletionSource<ByteString>> pending = [];
  private readonly ChannelWriter<PluginFrame>? output;
  private readonly IReadOnlyDictionary<string, ByteString> requires;
  private readonly CancellationTokenSource cancellation;
  private readonly TaskCompletionSource cancelled = new(TaskCreationOptions.RunContinuationsAsynchronously);
  private ulong nextCall;
  private string? cancelReason;

  internal Context(Request request, string plugin, IReadOnlyDictionary<string, ByteString> requires,
      CancellationTokenSource cancellation, ChannelWriter<PluginFrame>? output = null)
  {
    Operation = request.Operation;
    Invocation = request.Invocation;
    Attempt = request.Attempt;
    Scope = request.Scope;
    Plugin = plugin;
    this.requires = requires;
    this.cancellation = cancellation;
    this.output = output;
  }

  public string Operation { get; }
  public string Invocation { get; }
  public uint Attempt { get; }
  public string Scope { get; }
  public string Plugin { get; }
  public ulong Sequence { get; internal set; }
  public Timestamp? At { get; internal set; }
  public CancellationToken CancellationToken => cancellation.Token;

  public Task CancelledAsync() => cancelled.Task;

  public async Task<TResponse> CallAsync<TRequest, TResponse>(Operation<TRequest, TResponse> operation, TRequest request,
      OperationPolicy? policy = null)
      where TRequest : IMessage<TRequest>, new()
      where TResponse : IMessage<TResponse>, new()
  {
    if (output is null)
      throw new InvalidOperationException("Observers Cannot Call Operations");
    var contract = operation.Contract;
    if (!requires.TryGetValue(contract.Id, out var fingerprint) || fingerprint != contract.Fingerprint)
      throw new InvalidOperationException($"Undeclared Requirement: {contract.Id}");

    TaskCompletionSource<ByteString> completion;
    ulong call;
    lock (pending)
    {
      if (cancelReason is not null)
        throw CancelFailure(cancelReason);
      call = ++nextCall;
      completion = new(TaskCreationOptions.RunContinuationsAsynchronously);
      pending.Add(call, completion);
      if (!output.TryWrite(new PluginFrame
      {
        Child = new ChildRequest
        {
          Call = call,
          Operation = contract.Id,
          Payload = request.ToByteString(),
          Fingerprint = fingerprint,
          Policy = policy?.Clone()
        }
      }))
        throw new RpcException(new Status(StatusCode.Unavailable, "Host Closed"));
    }
    var payload = await completion.Task.WaitAsync(CancellationToken);
    try
    {
      return Operation<TRequest, TResponse>.DecodeResponse(payload);
    }
    catch (InvalidProtocolBufferException error)
    {
      throw new RpcException(new Status(StatusCode.InvalidArgument, error.Message));
    }
  }

  public Task<SleepResponse> SleepAsync(Duration duration) => CallAsync(Clock.Sleep, new Sleep { Duration = duration });

  public async Task<Timestamp> NowAsync()
  {
    var time = await CallAsync(Clock.Now, new Now());
    return time.Now;
  }

  internal void Reply(ChildResponse child)
  {
    lock (pending)
    {
      if (child.ResultCase == ChildResponse.ResultOneofCase.None || !pending.Remove(child.Call, out var completion))
        throw new RpcException(new Status(StatusCode.InvalidArgument, "Unknown, Duplicate or Empty Child Response"));
      if (child.ResultCase == ChildResponse.ResultOneofCase.Failure)
        completion.SetException(new PluginFailureException(child.Failure));
      else
        completion.SetResult(child.Payload);
    }
  }

  internal void Cancel(string reason)
  {
    lock (pending)
    {
      cancelReason = reason;
      foreach (var (call, completion) in pending)
      {
        output?.TryWrite(new PluginFrame { ChildCancel = new ChildCancel { Call = call } });
        completion.TrySetException(CancelFailure(reason));
      }
      pending.Clear();
    }
    cancelled.TrySetResult();
    cancellation.Cancel();
  }

  internal void Disconnected()
  {
    lock (pending)
    {
      foreach (var completion in pending.Values)
        completion.TrySetException(new RpcException(new Status(StatusCode.Unavailable, "Host Closed")));
      pending.Clear();
    }
    cancelled.TrySetResult();
    cancellation.Cancel();
  }

  internal bool HasPendingCalls { get { lock (pending) return pending.Count != 0; } }

  internal PluginFailureException CancelFailure(string reason) => new(new Failure
  {
    Classification = Classification.Cancelled,
    Code = "cancelled",
    Message = reason,
    Action = "Retry If Needed",
    Operation = Operation,
    Plugin = Plugin
  });

  internal PluginFailureException CancellationFailure() => CancelFailure(cancelReason ?? "Demand Closed");
}

/// <summary>Declare these typed requirements before using Context.SleepAsync or Context.NowAsync.</summary>
public static class Clock
{
  public static Operation<Sleep, SleepResponse> Sleep { get; } = new("harmos.clock.sleep", Visibility.Public);
  public static Operation<Now, Time> Now { get; } = new("harmos.clock.now", Visibility.Public);
}
