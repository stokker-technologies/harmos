// SPDX-License-Identifier: Apache-2.0

using System.Collections.Generic;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Threading.Channels;
using System.Threading.Tasks;
using System.Threading;
using System;
using Google.Protobuf.WellKnownTypes;
using Google.Protobuf;
using Grpc.Core;
using Harmos.Clients;
using Harmos.Internal.Plugin;
using Harmos;

namespace Harmos.Plugins;

/// <summary>One invocation or publication demand. All timed waits use the host clock.</summary>
public sealed class Context
{
  private Dictionary<ulong, TaskCompletionSource<ByteString>> pending = [];
  private readonly ChannelWriter<InstanceInvokeResponse>? output;
  private readonly IReadOnlyDictionary<string, ByteString> requires;
  private readonly CancellationTokenSource cancellation;
  private readonly TaskCompletionSource<string> cancelled = new(TaskCreationOptions.RunContinuationsAsynchronously);
  private readonly Dictionary<System.Type, object> resources = [];
  private Context root;
  private volatile bool windDownRequested;
  private readonly HashSet<ulong> owned = [];
  private readonly List<Context> children = [];
  private ulong nextCall;
  private readonly Dictionary<ulong, (Channel<ChildResponse> Channel, ulong Limit, Context Owner)> watches = [];
  private readonly HashSet<ulong> retired = [];
  private readonly List<Task> cancellationWrites = [];
  private string? cancelReason;

  internal Context(Request request, string plugin, IReadOnlyDictionary<string, ByteString> requires,
      CancellationTokenSource cancellation, ChannelWriter<InstanceInvokeResponse>? output = null)
  {
    OperationId = request.Operation;
    Invocation = request.Invocation;
    CapturingExecutions = request.CapturingExecutions.ToArray();
    Attempt = request.Attempt;
    scope = request.Scope;
    Plugin = plugin;
    this.requires = requires;
    this.cancellation = cancellation;
    this.output = output;
    root = this;
  }

  public IReadOnlyList<string> CapturingExecutions { get; internal set; } = [];

  public HostRoot Host => new(this);
  public HostPluginsStateResource State => Host.Plugins.State;

  public async Task<Subscription<T>> SubscribeAsync<T>(string stream, ulong maxItemBytes, CancellationToken cancellationToken = default)
      where T : IMessage<T>, new()
  {
    var subscription = new Subscription<T>(this, stream, maxItemBytes, cancellationToken);
    await subscription.OpenAsync();
    return subscription;
  }

  public string OperationId
  {
    get;
  }
  public string Invocation
  {
    get;
  }
  public uint Attempt
  {
    get; private set;
  }
  private readonly string scope;

  public string Scope()
  {
    if (scope.Length == 0)
      throw new OperationException(new Failure
      {
        Classification = Classification.Refused,
        Code = "scope_required",
        Message = "Invocation Scope Is Required: the caller must bind a scope"
      });
    return scope;
  }
  public string Plugin
  {
    get;
  }
  public ulong Sequence
  {
    get; internal set;
  }
  public Timestamp? At
  {
    get; internal set;
  }
  public CancellationToken CancellationToken => cancellation.Token;

  public Task<string> CancelledAsync() => cancelled.Task;

  public bool WindDownRequested => root.windDownRequested;

  internal void RequestWindDown() => root.windDownRequested = true;

  public void Set<T>(T value) where T : notnull
  {
    lock (resources)
      resources[typeof(T)] = value;
  }

  public T Get<T>() where T : notnull
  {
    lock (resources)
      return resources.TryGetValue(typeof(T), out var value) ? (T)value
          : throw new InvalidOperationException($"Missing Invocation Resource: {typeof(T).Name}");
  }

  public async Task<TResponse> Operation<TRequest, TResponse>(Operation<TRequest, TResponse> operation, TRequest request,
      OperationPolicy? policy = null, CancellationToken cancellationToken = default)
      where TRequest : IMessage<TRequest>, new()
      where TResponse : IMessage<TResponse>, new()
  {
    if (output is null)
      throw new InvalidOperationException($"Call {operation.Contract.Id}: Call Only While a Window or Invocation Is Unanswered");
    var contract = operation.Contract;
    if (operation.IsWatch)
      throw new InvalidOperationException($"Call {contract.Id}: Use Watch for a Streaming Contract");
    var child = new ChildRequest { Operation = contract.Id, Payload = request.ToByteString(), Fingerprint = contract.Fingerprint, Policy = policy?.Clone() };
    var payload = await SendAsync(child, cancellationToken);
    try
    {
      return global::Harmos.Plugins.Operation<TRequest, TResponse>.DecodeResponse(payload);
    }
    catch (InvalidProtocolBufferException error)
    {
      throw new RpcException(new global::Grpc.Core.Status(StatusCode.InvalidArgument, error.Message));
    }
  }

  /// <summary>Call any visible served operation using JSON; the host owns Protobuf conversion.</summary>
  public async Task<JsonNode?> Operation(string operation, object? payload = null, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var child = new ChildRequest { Operation = operation, Json = JsonSerializer.Serialize(payload ?? new { }), Policy = policy?.Clone() };
    var reply = await SendAsync(child, cancellationToken);
    return JsonNode.Parse(reply.ToStringUtf8());
  }

  private async Task<ByteString> SendAsync(ChildRequest child, CancellationToken cancellationToken)
  {
    if (output is null)
      throw new InvalidOperationException($"Call {child.Operation}: Call Only While a Window or Invocation Is Unanswered");
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
      child.Call = call;
      if (!output.TryWrite(new InstanceInvokeResponse { Child = child }))
        throw new RpcException(new global::Grpc.Core.Status(StatusCode.Unavailable, "Host Closed"));
    }
    using var registration = cancellationToken.Register(() =>
    {
      lock (pending)
      {
        if (!pending.Remove(call))
          return;
        owned.Remove(call);
        root.retired.Add(call);
        root.cancellationWrites.Add(output.WriteAsync(new InstanceInvokeResponse { ChildCancel = new ChildCancel { Call = call } }).AsTask());
        completion.TrySetCanceled(cancellationToken);
      }
    });
    ByteString payload;
    try
    {
      payload = await completion.Task;
    }
    finally { lock (pending) owned.Remove(call); }
    return payload;
  }

  public async IAsyncEnumerable<TResponse> WatchAsync<TRequest, TResponse>(Operation<TRequest, TResponse> operation,
      TRequest request, ulong maxItemBytes, [EnumeratorCancellation] CancellationToken cancellationToken = default)
      where TRequest : IMessage<TRequest>, new() where TResponse : IMessage<TResponse>, new()
  {
    var contract = operation.Contract;
    if (!operation.IsWatch || output is null || !requires.TryGetValue(contract.Id, out var fingerprint) || fingerprint != contract.Fingerprint || maxItemBytes == 0)
      throw new InvalidOperationException($"Watch {contract.Id}: Declared Requirement and Positive Item Limit Required");
    var channel = Channel.CreateBounded<ChildResponse>(1);
    ulong call;
    lock (pending)
    {
      if (cancelReason is not null)
        throw CancelFailure(cancelReason);
      call = ++root.nextCall;
      root.watches.Add(call, (channel, maxItemBytes, this));
    }
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
        bool active;
        lock (pending)
          active = root.watches.ContainsKey(call);
        if (active)
          await output.WriteAsync(new InstanceInvokeResponse { ChildDemand = new ChildDemand { Call = call } }, cancellationToken);
        ChildResponse reply;
        try
        {
          reply = await channel.Reader.ReadAsync(cancellationToken);
        }
        catch (ChannelClosedException error) when (error.InnerException is OperationException failure)
        {
          throw failure;
        }
        if (reply.ResultCase == ChildResponse.ResultOneofCase.Failure)
          throw new OperationException(reply.Failure);
        if (reply.ResultCase == ChildResponse.ResultOneofCase.Payload)
          yield break;
        yield return global::Harmos.Plugins.Operation<TRequest, TResponse>.DecodeResponse(reply.Item);
      }
    }
    finally
    {
      bool removed;
      lock (pending)
      {
        removed = root.watches.Remove(call);
        if (removed)
          root.retired.Add(call);
      }
      channel.Writer.TryComplete();
      if (removed)
        await output.WriteAsync(new InstanceInvokeResponse { ChildCancel = new ChildCancel { Call = call } });
    }
  }

  internal async Task Reply(ChildResponse child)
  {
    (Channel<ChildResponse> Channel, ulong Limit, Context Owner) watching;
    lock (pending)
    {
      if (child.ResultCase == ChildResponse.ResultOneofCase.None)
        throw new RpcException(new global::Grpc.Core.Status(StatusCode.InvalidArgument, "Reply to Child: Result Is Required"));
      if (root.retired.Contains(child.Call))
        return;
      root.watches.TryGetValue(child.Call, out watching);
      if (watching.Channel is not null && child.ResultCase != ChildResponse.ResultOneofCase.Item)
        root.watches.Remove(child.Call);
    }
    if (watching.Channel is not null)
    {
      if ((ulong)child.Item.Length > watching.Limit || (ulong)child.Payload.Length > watching.Limit)
        throw new RpcException(new global::Grpc.Core.Status(StatusCode.ResourceExhausted, "Watch: Item Exceeds Consumer Limit"));
      await watching.Channel.Writer.WriteAsync(child);
      return;
    }
    lock (pending)
    {
      if (child.ResultCase is ChildResponse.ResultOneofCase.None or ChildResponse.ResultOneofCase.Item || !pending.Remove(child.Call, out var completion))
        throw new RpcException(new global::Grpc.Core.Status(StatusCode.InvalidArgument, "Unknown, Duplicate or Empty Child Response"));
      if (child.ResultCase == ChildResponse.ResultOneofCase.Failure)
        completion.SetException(new OperationException(child.Failure));
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
      if (cancelReason is not null)
        return;
      cancelReason = reason;
      nested = children.ToArray();
      foreach (var call in owned)
        if (pending.Remove(call, out var complete))
        {
          calls.Add((call, complete));
          root.retired.Add(call);
        }
      owned.Clear();
      foreach (var (call, watch) in root.watches.ToArray())
        if (ReferenceEquals(root, this) || ReferenceEquals(watch.Owner, this))
        {
          root.watches.Remove(call);
          root.retired.Add(call);
          streams.Add((call, watch.Channel));
        }
    }
    foreach (var child in nested)
      await child.Cancel(reason);
    foreach (var (call, complete) in calls)
    {
      if (output is not null)
        await output.WriteAsync(new InstanceInvokeResponse { ChildCancel = new ChildCancel { Call = call } });
      complete.TrySetException(CancelFailure(reason));
    }
    foreach (var (call, stream) in streams)
    {
      if (output is not null)
        await output.WriteAsync(new InstanceInvokeResponse { ChildCancel = new ChildCancel { Call = call } });
      stream.Writer.TryComplete(CancelFailure(reason));
    }
    cancelled.TrySetResult(reason);
    await cancellation.CancelAsync();
  }

  internal async Task FlushCancellations()
  {
    Task[] writes;
    lock (pending)
    {
      writes = root.cancellationWrites.ToArray();
      root.cancellationWrites.Clear();
    }
    await Task.WhenAll(writes);
  }

  /// <summary>One sample of the unanswered window; its children share the window's call IDs and lifetime.</summary>
  internal Context Sample(ulong sequence, Timestamp at)
  {
    var context = new Context(new Request { Operation = OperationId, Invocation = Invocation, Scope = scope, Attempt = Attempt },
        Plugin, requires, cancellation, output)
    {
      pending = pending,
      root = root,
      CapturingExecutions = CapturingExecutions,
      Sequence = sequence,
      At = at
    };
    lock (pending)
    {
      context.cancelReason = cancelReason;
      children.Add(context);
    }
    if (context.cancelReason is not null)
      context.cancelled.TrySetResult(context.cancelReason);
    return context;
  }

  internal void Disconnected()
  {
    Context[] nested;
    lock (pending)
    {
      cancelReason ??= "Connection Closed";
      foreach (var completion in pending.Values)
        completion.TrySetException(new RpcException(new global::Grpc.Core.Status(StatusCode.Unavailable, "Host Closed")));
      pending.Clear();
      foreach (var watching in root.watches.Values)
        watching.Channel.Writer.TryComplete(new RpcException(new global::Grpc.Core.Status(StatusCode.Unavailable, "Watch: Host Closed")));
      root.watches.Clear();
      nested = children.ToArray();
    }
    // Cancel Before Releasing Waiters, So a Released Observer Sees Its Demand Closed.
    cancellation.Cancel();
    foreach (var child in nested)
      child.Released("Connection Closed");
    cancelled.TrySetResult("Connection Closed");
  }

  private void Released(string reason)
  {
    Context[] nested;
    lock (pending)
    {
      cancelReason ??= reason;
      nested = children.ToArray();
    }
    foreach (var child in nested)
      child.Released(reason);
    cancelled.TrySetResult(reason);
  }

  internal bool HasPendingCalls
  {
    get
    {
      lock (pending)
        return pending.Count != 0 || root.watches.Count != 0;
    }
  }

  internal ulong LastCall
  {
    get => root.nextCall; set => root.nextCall = value;
  }

  internal OperationException CancelFailure(string reason) => new(new Failure
  {
    Classification = Classification.Cancelled,
    Code = "cancelled",
    Message = reason,
    Action = "Retry If Needed",
    Operation = OperationId,
    Plugin = Plugin
  });

  internal OperationException CancellationFailure() => CancelFailure(cancelReason ?? "Demand Closed");
}
