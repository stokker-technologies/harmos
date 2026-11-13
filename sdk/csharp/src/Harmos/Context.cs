using Status = Grpc.Core.Status;
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
  private Dictionary<ulong, TaskCompletionSource<ByteString>> pending = [];
  private readonly ChannelWriter<InstanceInvokeResponse>? output;
  private readonly IReadOnlyDictionary<string, ByteString> requires;
  private readonly CancellationTokenSource cancellation;
  private readonly TaskCompletionSource<string> cancelled = new(TaskCreationOptions.RunContinuationsAsynchronously);
  private readonly Dictionary<System.Type, object> resources = [];
  private readonly IReadOnlyDictionary<string, Handler> local;
  private Context root;
  private readonly HashSet<ulong> owned = [];
  private readonly List<Context> children = [];
  private ulong nextCall;
  private readonly Dictionary<ulong, (Channel<ChildResponse> Channel, ulong Limit, Context Owner)> watches = [];
  private bool background;
  private readonly HashSet<ulong> retired = [];
  private readonly List<Task> cancellationWrites = [];
  private string? cancelReason;

  internal Context(Request request, string plugin, IReadOnlyDictionary<string, ByteString> requires,
      CancellationTokenSource cancellation, ChannelWriter<InstanceInvokeResponse>? output = null, IReadOnlyDictionary<string, Handler>? local = null)
  {
    Operation = request.Operation;
    Invocation = request.Invocation;
    Attempt = request.Attempt;
    scope = request.Scope;
    Plugin = plugin;
    this.requires = requires;
    this.cancellation = cancellation;
    this.output = output;
    this.local = local ?? new Dictionary<string, Handler>();
    root = this;
  }

  public string Operation { get; }
  public string Invocation { get; }
  public uint Attempt { get; private set; }
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
      OperationPolicy? policy = null, CancellationToken cancellationToken = default)
      where TRequest : IMessage<TRequest>, new()
      where TResponse : IMessage<TResponse>, new()
  {
    if (output is null)
      throw new InvalidOperationException("Observers Cannot Call Operations");
    var contract = operation.Contract;
    if (operation.IsWatch) throw new InvalidOperationException($"Call {contract.Id}: Use Watch for a Streaming Contract");
    if (local.TryGetValue(contract.Id, out var handler) && handler.Contract.Visibility == Visibility.Private)
    {
      if (handler.Contract.Fingerprint != contract.Fingerprint)
        throw new InvalidOperationException($"Invoke {contract.Id}: Local Contract Differs");
      return Operation<TRequest, TResponse>.DecodeResponse(await InvokeLocal(handler, request, policy));
    }
    if (!requires.TryGetValue(contract.Id, out var fingerprint) || fingerprint != contract.Fingerprint)
      throw new InvalidOperationException($"Undeclared Requirement: {contract.Id}");

    TaskCompletionSource<ByteString> completion;
    ulong call;
    lock (pending)
    {
      if (cancelReason is not null)
        throw CancelFailure(cancelReason);
      call = ++root.nextCall;
      owned.Add(call);
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
          Policy = policy?.Clone(),
          Background = background
        }
      }))
        throw new RpcException(new Status(StatusCode.Unavailable, "Host Closed"));
    }
    using var registration = cancellationToken.Register(() =>
    {
      lock (pending)
      {
        if (!pending.Remove(call)) return;
        owned.Remove(call);
        root.retired.Add(call);
        root.cancellationWrites.Add(output.WriteAsync(new InstanceInvokeResponse { ChildCancel = new ChildCancel { Call = call } }).AsTask());
        completion.TrySetCanceled(cancellationToken);
      }
    });
    ByteString payload;
    try { payload = await completion.Task; }
    finally { lock (pending) owned.Remove(call); }
    try
    {
      return Operation<TRequest, TResponse>.DecodeResponse(payload);
    }
    catch (InvalidProtocolBufferException error)
    {
      throw new RpcException(new Status(StatusCode.InvalidArgument, error.Message));
    }
  }

  public async IAsyncEnumerable<TResponse> WatchAsync<TRequest, TResponse>(Operation<TRequest, TResponse> operation,
      TRequest request, ulong maxItemBytes, [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken = default)
      where TRequest : IMessage<TRequest>, new() where TResponse : IMessage<TResponse>, new()
  {
    var contract = operation.Contract;
    if (!operation.IsWatch || output is null || !requires.TryGetValue(contract.Id, out var fingerprint) || fingerprint != contract.Fingerprint || maxItemBytes == 0)
      throw new InvalidOperationException($"Watch {contract.Id}: Declared Requirement and Positive Item Limit Required");
    var channel = Channel.CreateBounded<ChildResponse>(1);
    ulong call;
    lock (pending) { call = ++root.nextCall; root.watches.Add(call, (channel, maxItemBytes, this)); }
    try
    {
      await output.WriteAsync(new InstanceInvokeResponse
      {
        Child = new ChildRequest
        {
          Call = call,
          Operation = contract.Id,
          Fingerprint = fingerprint,
          Payload = request.ToByteString(),
          Watch = true,
          MaxResponseBytes = maxItemBytes
        }
      }, cancellationToken);
      while (true)
      {
        await output.WriteAsync(new InstanceInvokeResponse { ChildDemand = new ChildDemand { Call = call } }, cancellationToken);
        var reply = await channel.Reader.ReadAsync(cancellationToken);
        if (reply.ResultCase == ChildResponse.ResultOneofCase.Failure) throw new PluginFailureException(reply.Failure);
        if (reply.ResultCase == ChildResponse.ResultOneofCase.Payload) yield break;
        yield return Operation<TRequest, TResponse>.DecodeResponse(reply.Item);
      }
    }
    finally
    {
      bool removed;
      lock (pending) { removed = root.watches.Remove(call); if (removed) root.retired.Add(call); }
      channel.Writer.TryComplete();
      if (removed) await output.WriteAsync(new InstanceInvokeResponse { ChildCancel = new ChildCancel { Call = call } });
    }
  }

  private async Task<ByteString> InvokeLocal(Handler handler, IMessage request, OperationPolicy? overridePolicy)
  {
    var contract = handler.Contract;
    var policy = overridePolicy ?? contract.Policy ?? new OperationPolicy();
    Int128? timeout = policy.Timeout is null ? null : Nanos(policy.Timeout, contract.Id);
    Int128? attemptTimeout = policy.AttemptTimeout is null ? null : Nanos(policy.AttemptTimeout, contract.Id);
    var grace = policy.CancellationTimeout is null ? 0 : Nanos(policy.CancellationTimeout, contract.Id);
    var attempts = policy.Retries?.TotalAttempts ?? 1;
    if (timeout <= 0 || attemptTimeout <= 0 || grace < 0 || attempts == 0 || attempts > 1 && !contract.ReplaySafe)
      throw new InvalidOperationException($"Invoke {contract.Id}: Invalid Policy or Replay Safety");
    Context? active = null;
    if (attempts > 1)
    {
      var initial = policy.Retries!.Initial is null ? 0 : Nanos(policy.Retries.Initial, contract.Id);
      var maximum = policy.Retries.Maximum is null ? initial : Nanos(policy.Retries.Maximum, contract.Id);
      if (initial <= 0 || maximum < initial) throw new InvalidOperationException($"Invoke {contract.Id}: Invalid Retry Delay");
    }
    var guard = new Context(new Request { Operation = Operation, Invocation = Invocation, Scope = scope, Attempt = Attempt },
        Plugin, requires, cancellation, output, local)
    { pending = pending, root = root, background = true };
    var started = timeout.HasValue ? TimestampNanos(await NowAsync()) : 0;
    try
    {
      for (ulong attempt = 0; attempt < attempts; ++attempt)
      {
        if (cancelReason is not null) throw CancelFailure(cancelReason);
        var remaining = timeout.HasValue ? timeout.Value - (TimestampNanos(await NowAsync()) - started) : (Int128?)null;
        var bound = remaining.HasValue && attemptTimeout.HasValue ? (remaining.Value < attemptTimeout.Value ? remaining : attemptTimeout) : remaining ?? attemptTimeout;
        if (bound <= 0) throw Interrupted(Classification.TimedOut, contract.Id, "Deadline Reached");
        using var stopped = CancellationTokenSource.CreateLinkedTokenSource(cancellation.Token);
        var context = new Context(new Request { Operation = contract.Id, Invocation = Invocation, Scope = scope, Attempt = checked((uint)attempt + 1) },
            Plugin, requires, stopped, output, local)
        { pending = pending, root = root };
        active = context;
        lock (pending) children.Add(context);
        using var timerStopped = new CancellationTokenSource();
        var executing = handler.Execute(context, request);
        var timer = bound.HasValue ? guard.CallAsync(Clock.Sleep, new Harmos.Context.V1.TimeSleepRequest { Duration = DurationNanos(bound.Value) }, cancellationToken: timerStopped.Token) : Task.Delay(Timeout.Infinite, timerStopped.Token);
        try
        {
          var completed = await Task.WhenAny(executing, timer, CancelledAsync());
          if (completed != executing)
          {
            var reason = completed == timer ? "Deadline Reached" : await CancelledAsync();
            if (completed == timer) await timer;
            await context.Cancel(reason);
            var deadlineReached = completed == timer;
            if (grace > 0 && !deadlineReached)
            {
              var settling = guard.CallAsync(Clock.Sleep, new Harmos.Context.V1.TimeSleepRequest { Duration = DurationNanos(grace) }, cancellationToken: timerStopped.Token);
              deadlineReached = await Task.WhenAny(executing, settling, timer) == timer;
            }
            if (grace == 0 || deadlineReached || !executing.IsCompleted) throw Interrupted(completed == timer ? Classification.TimedOut : Classification.Cancelled, contract.Id, reason);
          }
          return await executing;
        }
        catch (PluginFailureException error) when (error.Failure.Classification == Classification.Transient && attempt + 1 < attempts)
        {
          await timerStopped.CancelAsync();
          await context.Cancel("Local Attempt Settled");
          var retry = policy.Retries!;
          var initial = retry.Initial is null ? 0 : Nanos(retry.Initial, contract.Id);
          Int128? maximum = retry.Maximum is null ? null : Nanos(retry.Maximum, contract.Id);
          if (initial <= 0 || maximum < initial) throw new InvalidOperationException($"Invoke {contract.Id}: Invalid Retry Delay");
          var nanos = (UInt128)initial;
          var cap = maximum.HasValue ? (UInt128)maximum.Value : nanos + nanos / 4;
          if (maximum.HasValue)
            for (ulong step = 0; step < attempt && nanos < cap; ++step) nanos = UInt128.Min(nanos * 2, cap);
          if (retry.DeterministicJitter) nanos = UInt128.Min(nanos + ((UInt128)attempt * 17 + 11) % (nanos / 4 + 1), cap);
          if (timeout.HasValue) nanos = UInt128.Min(nanos, (UInt128)Int128.Max(0, timeout.Value - (TimestampNanos(await NowAsync()) - started)));
          if (nanos > 0) await SleepAsync(new Duration { Seconds = checked((long)(nanos / 1_000_000_000)), Nanos = checked((int)(nanos % 1_000_000_000)) });
        }
        finally
        {
          await timerStopped.CancelAsync();
          await context.Cancel("Local Attempt Settled");
          lock (pending) children.Remove(context);
          active = null;
          await FlushCancellations();
        }
      }
      throw new InvalidOperationException($"Invoke {contract.Id}: No Attempt Admitted");
    }
    finally { if (active is not null) { await active.Cancel("Local Attempt Settled"); lock (pending) children.Remove(active); } }
  }

  private static Int128 Nanos(Duration value, string operation)
  {
    if (value.Seconds < 0 || value.Seconds > 315_576_000_000L || value.Nanos < 0 || value.Nanos >= 1_000_000_000)
      throw new InvalidOperationException($"Invoke {operation}: Invalid Policy Duration");
    return (Int128)value.Seconds * 1_000_000_000 + value.Nanos;
  }
  private static Int128 TimestampNanos(Timestamp value) => (Int128)value.Seconds * 1_000_000_000 + value.Nanos;
  private static Duration DurationNanos(Int128 value) => new() { Seconds = checked((long)(value / 1_000_000_000)), Nanos = checked((int)(value % 1_000_000_000)) };

  private static PluginFailureException Interrupted(Classification classification, string operation, string reason) => new(new Failure
  {
    Classification = classification,
    Operation = operation,
    Code = "interrupted",
    Message = $"Invoke {operation}: {reason}",
    Action = "Invoke Again If Safe"
  });

  public Task<Harmos.Context.V1.TimeSleepResponse> SleepAsync(Duration duration) => CallAsync(Clock.Sleep, new Harmos.Context.V1.TimeSleepRequest { Duration = duration });

  public async Task<Timestamp> NowAsync()
  {
    var time = await CallAsync(Clock.GetTime, new Harmos.Context.V1.TimeGetRequest());
    return time.Now;
  }

  internal async Task Reply(ChildResponse child)
  {
    (Channel<ChildResponse> Channel, ulong Limit, Context Owner) watching;
    lock (pending)
    {
      if (root.retired.Contains(child.Call)) return;
      root.watches.TryGetValue(child.Call, out watching);
      if (watching.Channel is not null && child.ResultCase != ChildResponse.ResultOneofCase.Item) root.watches.Remove(child.Call);
    }
    if (watching.Channel is not null)
    {
      if ((ulong)child.Item.Length > watching.Limit || (ulong)child.Payload.Length > watching.Limit)
        throw new RpcException(new Status(StatusCode.ResourceExhausted, "Watch: Item Exceeds Consumer Limit"));
      await watching.Channel.Writer.WriteAsync(child); return;
    }
    lock (pending)
    {
      if (child.ResultCase is ChildResponse.ResultOneofCase.None or ChildResponse.ResultOneofCase.Item || !pending.Remove(child.Call, out var completion))
        throw new RpcException(new Status(StatusCode.InvalidArgument, "Unknown, Duplicate or Empty Child Response"));
      if (child.ResultCase == ChildResponse.ResultOneofCase.Failure)
        completion.SetException(new PluginFailureException(child.Failure));
      else
        completion.SetResult(child.Payload);
    }
  }

  internal async Task Cancel(string reason)
  {
    Context[] nested;
    List<(ulong Call, TaskCompletionSource<ByteString> Complete)> calls = [];
    List<(ulong Call, Channel<ChildResponse> Channel)> streams = [];
    lock (pending)
    {
      if (cancelReason is not null) return;
      cancelReason = reason;
      nested = children.ToArray();
      foreach (var call in owned)
        if (pending.Remove(call, out var complete)) { calls.Add((call, complete)); root.retired.Add(call); }
      owned.Clear();
      foreach (var (call, watch) in root.watches.ToArray())
        if (ReferenceEquals(root, this) || ReferenceEquals(watch.Owner, this))
        {
          root.watches.Remove(call); root.retired.Add(call); streams.Add((call, watch.Channel));
        }
    }
    foreach (var child in nested) await child.Cancel(reason);
    foreach (var (call, complete) in calls)
    {
      if (output is not null) await output.WriteAsync(new InstanceInvokeResponse { ChildCancel = new ChildCancel { Call = call } });
      complete.TrySetException(CancelFailure(reason));
    }
    foreach (var (call, stream) in streams)
    {
      if (output is not null) await output.WriteAsync(new InstanceInvokeResponse { ChildCancel = new ChildCancel { Call = call } });
      stream.Writer.TryComplete(CancelFailure(reason));
    }
    cancelled.TrySetResult(reason);
    await cancellation.CancelAsync();
  }

  internal async Task FlushCancellations()
  {
    Task[] writes;
    lock (pending) { writes = root.cancellationWrites.ToArray(); root.cancellationWrites.Clear(); }
    await Task.WhenAll(writes);
  }

  internal void Disconnected()
  {
    lock (pending)
    {
      foreach (var completion in pending.Values)
        completion.TrySetException(new RpcException(new Status(StatusCode.Unavailable, "Host Closed")));
      pending.Clear();
      foreach (var watching in root.watches.Values) watching.Channel.Writer.TryComplete(new RpcException(new Status(StatusCode.Unavailable, "Watch: Host Closed")));
      root.watches.Clear();
    }
    cancelled.TrySetResult("Connection Closed");
    cancellation.Cancel();
  }

  internal bool HasPendingCalls { get { lock (pending) return pending.Count != 0 || root.watches.Count != 0; } }

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
  public static Operation<Harmos.Context.V1.TimeSleepRequest, Harmos.Context.V1.TimeSleepResponse> Sleep { get; } = new("harmos.context.v1.Time/Sleep", Visibility.Public);
  public static Operation<Harmos.Context.V1.TimeGetRequest, Harmos.Context.V1.TimeGetResponse> GetTime { get; } = new("harmos.context.v1.Time/Get", Visibility.Public);
}
