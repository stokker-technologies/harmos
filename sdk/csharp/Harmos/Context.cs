using Harmos.Context.V1;
using Harmos.Plugin.V1;
// SPDX-License-Identifier: Apache-2.0

using System.Threading.Channels;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Harmos.V1;

namespace Harmos.Sdk;

/// <summary>One invocation or publication demand. All timed waits use the host clock.</summary>
public sealed class Context
{
  private readonly Dictionary<ulong, TaskCompletionSource<ByteString>> pending = [];
  private readonly ChannelWriter<InstanceInvokeResponse>? output;
  private readonly IReadOnlyDictionary<string, ByteString> requires;
  private readonly CancellationTokenSource cancellation;
  private readonly TaskCompletionSource<string> cancelled = new(TaskCreationOptions.RunContinuationsAsynchronously);
  private readonly Dictionary<System.Type, object> resources = [];
  private ulong nextCall;
  private string? cancelReason;

  internal Context(Request request, string plugin, IReadOnlyDictionary<string, ByteString> requires,
      CancellationTokenSource cancellation, ChannelWriter<InstanceInvokeResponse>? output = null)
  {
    Operation = request.Operation;
    Invocation = request.Invocation;
    Attempt = request.Attempt;
    scope = request.Scope;
    Plugin = plugin;
    this.requires = requires;
    this.cancellation = cancellation;
    this.output = output;
  }

  public string Operation { get; }
  public string Invocation { get; }
  public uint Attempt { get; }
  private readonly string scope;

  public string Scope()
  {
    if (scope.Length == 0)
      throw new PluginFailureException(new Failure
      {
        Classification = Classification.Refused,
        Code = "scope_required",
        Message = "Invocation Scope Is Required: the caller must bind a scope"
      });
    return scope;
  }
  public string Plugin { get; }
  public ulong Sequence { get; internal set; }
  public Timestamp? At { get; internal set; }
  public CancellationToken CancellationToken => cancellation.Token;

  public Task<string> CancelledAsync() => cancelled.Task;

  public void Set<T>(T value) where T : notnull
  {
    lock (resources) resources[typeof(T)] = value;
  }

  public T Get<T>() where T : notnull
  {
    lock (resources)
      return resources.TryGetValue(typeof(T), out var value) ? (T)value
          : throw new InvalidOperationException($"Missing Invocation Resource: {typeof(T).Name}");
  }

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
      if (!output.TryWrite(new InstanceInvokeResponse
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
    var payload = await completion.Task;
    try
    {
      return Operation<TRequest, TResponse>.DecodeResponse(payload);
    }
    catch (InvalidProtocolBufferException error)
    {
      throw new RpcException(new Status(StatusCode.InvalidArgument, error.Message));
    }
  }

  public Task<TimeSleepResponse> SleepAsync(Duration duration) => CallAsync(Clock.Sleep, new TimeSleepRequest { Duration = duration });

  public async Task<Timestamp> NowAsync()
  {
    var time = await CallAsync(Clock.GetTime, new TimeGetRequest());
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
      if (cancelReason is not null) return;
      cancelReason = reason;
      foreach (var (call, completion) in pending)
      {
        output?.TryWrite(new InstanceInvokeResponse { ChildCancel = new ChildCancel { Call = call } });
        completion.TrySetException(CancelFailure(reason));
      }
      pending.Clear();
    }
    cancelled.TrySetResult(reason);
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
    cancelled.TrySetResult("Connection Closed");
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
  public static Operation<TimeSleepRequest, TimeSleepResponse> Sleep { get; } = new("harmos.time.sleep", Visibility.Public);
  public static Operation<TimeGetRequest, TimeGetResponse> GetTime { get; } = new("harmos.time.get", Visibility.Public);
}
