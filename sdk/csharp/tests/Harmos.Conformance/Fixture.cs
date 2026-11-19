// SPDX-License-Identifier: Apache-2.0

using System.Buffers.Binary;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Threading;
using System;
using Google.Protobuf.WellKnownTypes;
using Google.Protobuf;
using Harmos.Clients;
using Harmos.Plugins;
using Harmos;
using Harmos.Conformance;

namespace Harmos.Conformance;

public sealed class Fixture
{
  private readonly Operation<Value, Value> peerPublic = new("conformance.peer.public", Visibility.Public);
  private readonly Operation<Value, Value> peerProtected = new("conformance.peer.protected", Visibility.Protected);
  private readonly Operation<Value, Value> peerLocal = new("conformance.peer.local", Visibility.Protected);
  private readonly Operation<Value, Value> ownProtected = new("example.plugin.v1.Lifecycle/Initialize", Visibility.Protected);
  private readonly Operation<Value, Value> hostWatch = new("conformance.host.watch", Visibility.Public, watch: true);
  private readonly Operation<Value, Value> local = new("conformance.local", Visibility.Private, replaySafe: true);
  private readonly Operation<Value, Value> hostEcho = new("conformance.host.echo", Visibility.Public);
  private readonly Operation<Value, Value> hostPending = new("conformance.host.pending", Visibility.Public);
  private readonly Operation<Value, Value> hostFail = new("conformance.host.fail", Visibility.Public);
  private readonly Operation<Wire.InvocationsCreateRequest, Wire.InvocationsCreateResponse> createInvocation = new("application.host.v1.Invocations/Create", Visibility.Public);

  private static readonly Lane Instrument = Lane.Serial("instrument");
  private static readonly Lane Parallel = Lane.Concurrent("parallel", 2);
  private static readonly Lane Open = Lane.Unbounded("open");
  private static readonly Lane Other = Lane.Serial("other");

  public global::Harmos.Plugins.Plugin Build() => new global::Harmos.Plugins.Plugin("conformance", "1.0.0")
      .Lane(Instrument).Lane(Parallel).Lane(Open).Lane(Other).Register(Register);

  private void Register(Registrar ctx)
  {
    ctx.Register(Catalog());
  }

  public Registration Catalog() => ctx =>
  {
    ctx.Register(new global::Harmos.Plugins.Descriptor<PayloadOnly>());
    ctx.Register(new Operation<Value, Value>("conformance.lane.first", Visibility.Public, lane: Instrument), Cancel);
    ctx.Register(new Operation<Value, Value>("conformance.lane.second", Visibility.Public, lane: Instrument), Cancel);
    ctx.Register(new Operation<Value, Value>("conformance.lane.parallel", Visibility.Public, lane: Parallel), Cancel);
    ctx.Register(new Operation<Value, Value>("conformance.lane.other", Visibility.Public, lane: Other), Cancel);
    ctx.Register(new Operation<Value, Value>("conformance.lane.open", Visibility.Public), Cancel);
    ctx.Register(local, Local);
    ctx.Register(new Operation<Value, Value>("conformance.local_call", Visibility.Public), LocalCall);
    ctx.Register(new Operation<Value, Value>("conformance.step.0", Visibility.Public), Echo);
    ctx.Register(new Operation<Value, Value>("conformance.children", Visibility.Public), Children);
    ctx.Register(new Operation<Value, Value>("conformance.cancel", Visibility.Public), Cancel);
    ctx.Register(new Operation<Value, Value>("conformance.failure", Visibility.Public), Failure);
    ctx.Register(new Operation<Value, Value>("conformance.child_failure", Visibility.Public), ChildFailure);
    ctx.Register(new Operation<Value, Value>("conformance.policy", Visibility.Public), Policy);
    ctx.Register(new Operation<Graph, Graph.Types.Nested>("conformance.schema", Visibility.Public),
          (context, request) => Task.FromResult(new Graph.Types.Nested { Count = request.Items.Count, Next = request }));
    ctx.Register(new Operation<Graph.Types.Nested, Graph>("conformance.nested", Visibility.Public),
          (context, request) => Task.FromResult(request.Next ?? new Graph()));
    ctx.Register(new Operation<Graph, Value>("conformance.projected", Visibility.Public),
          (context, request) => Task.FromResult(request.Tail?.Value ?? new Value()));
    ctx.Register(new Operation<Value, Value>("conformance.clock", Visibility.Public), ClockCall);
    ctx.Register(new Operation<Value, Value>("example.plugin.v1.Lifecycle/Initialize", Visibility.Protected), Echo);
    ctx.Register(new Operation<Value, Value>("example.plugin.v1.Steps/Execute", Visibility.Public, replaySafe: true), Resources);
    ctx.Register(new Operation<Value, Value>("conformance.cancel_reason", Visibility.Public), CancelReason);
    ctx.Register(new Operation<Value, Value>("conformance.watch", Visibility.Public), Watch);
    ctx.Register(new Operation<Value, Value>("conformance.peer_call", Visibility.Public), PeerCall);
    ctx.Register(new Operation<Value, Value>("conformance.settings_read", Visibility.Public), ReadSettings);
    ctx.Register(new Operation<Value, Value>("conformance.discover", Visibility.Public), Discover);
    ctx.Register(new Operation<Value, Value>("conformance.caller_refusal", Visibility.Public), CallerRefusal);
    ctx.Register(createInvocation);
    ctx.Register(peerPublic);
    ctx.Register(peerProtected);
    ctx.Register(peerLocal);
    ctx.Register(ownProtected);
    ctx.Register(hostWatch);
    ctx.Register(hostEcho);
    ctx.Register(hostPending);
    ctx.Register(hostFail);
    ctx.Register(new Operation<Graph, Value>("conformance.host.schema", Visibility.Public));
    ctx.Register(HostOperation.TimeSleep);
    ctx.Register(HostOperation.TimeGet);
    ctx.Register(HostOperation.PluginsSettingsList);
    ctx.Register(HostOperation.OperationsList);
    ctx.Register(HostOperation.OperationsGet);
    ctx.Register("conformance.interval", new Duration { Seconds = 1 }, Sequence);
    ctx.Register<Value>("conformance.device", Device);
    ctx.Register("conformance.settings", new Duration { Seconds = 1 }, Sequence, intervalSetting: "interval");
    ctx.Register("conformance.window_children", new Duration { Seconds = 1 }, context => Children(context, new Value()));
    ctx.Register("conformance.window_context", new Duration { Seconds = 1 }, SampleContext);
    ctx.Register("conformance.window_failure", new Duration { Seconds = 1 }, SampleFailure);
    ctx.Register("conformance.window_watch", new Duration { Seconds = 1 }, context => Watch(context, new Value { Value_ = ByteString.CopyFromUtf8("fixture") }));
    ctx.Register("conformance.window_cancel", new Duration { Seconds = 1 }, SampleCancel);
    ctx.Register("conformance.window_caller_refusal", new Duration { Seconds = 1 }, context => CallerRefusal(context, new Value()));
    ctx.Register<Value, Values>("conformance.window", new Duration { Seconds = 1 }, Sequences);
    ctx.Register("conformance.oversized", new Duration { Seconds = 1 }, Oversized);
    ctx.Register(new Graph { Interval = new Duration { Seconds = 2 } });
    ctx.Register("count", new SettingValue { Integer = 3 }, "count", "Conformance count");
    ctx.Register("enabled", new SettingValue { Flag = false }, "enabled", "Conformance enabled");
    ctx.Register("gain", new SettingValue { Number = 1.5 }, "gain", "Conformance gain");
    ctx.Register("interval", new SettingValue { Duration = new Duration { Seconds = 2 } }, "interval", "Conformance interval");
    ctx.Register("label", new SettingValue { Text = "default" }, "label", "Conformance label");
  };

  private async Task<Value> CallerRefusal(global::Harmos.Plugins.Context context, Value request)
  {
    try
    {
      var invocation = Wire.InvocationsCreateRequest.Parser.ParseFrom(CallerRequest.Bytes);
      await context.CallAsync(createInvocation, invocation);
    }
    catch (PluginFailureException error)
    {
      if (request.Value_ == ByteString.CopyFromUtf8("propagate"))
        throw;
      return new Value { Value_ = error.Failure.ToByteString() };
    }
    throw new InvalidOperationException("Create Invocation: Client-Only Operation Was Admitted");
  }

  private static async Task<Value> ReadSettings(global::Harmos.Plugins.Context context, Value request)
  {
    var settings = await context.Host.Plugins.Settings.List();
    return new Value { Value_ = settings.Value.ToByteString() };
  }

  private async Task<Value> Discover(global::Harmos.Plugins.Context context, Value request)
  {
    var operations = await context.Host.Operations.List();
    var id = operations[0].Contract.Id;
    var operation = await context.Host.Operations.Get(id);
    return new Value { Value_ = ByteString.CopyFromUtf8(operation.Contract.Id) };
  }

  private async Task<Value> SampleContext(global::Harmos.Plugins.Context context)
  {
    var echo = await context.CallAsync(hostEcho, new Value { Value_ = ByteString.CopyFromUtf8("fixture") });
    var clock = new TimeGetResponse { Now = await context.Host.Time.Get() };
    var settings = await context.Host.Plugins.Settings.List();
    using var value = new MemoryStream();
    foreach (var reply in new IMessage[] { echo, clock, settings })
    {
      var payload = reply.ToByteArray();
      var length = new byte[4];
      BinaryPrimitives.WriteUInt32LittleEndian(length, (uint)payload.Length);
      value.Write(length);
      value.Write(payload);
    }
    return new Value { Value_ = ByteString.CopyFrom(value.ToArray()) };
  }

  private async Task<Value> SampleFailure(global::Harmos.Plugins.Context context)
  {
    try
    {
      return await context.CallAsync(hostFail, new Value());
    }
    catch (PluginFailureException error) { return new Value { Value_ = error.Failure.ToByteString() }; }
  }

  private async Task<Value> SampleCancel(global::Harmos.Plugins.Context context)
  {
    using var cancelled = new CancellationTokenSource();
    var pending = context.CallAsync(hostPending, new Value(), cancellationToken: cancelled.Token);
    _ = pending.ContinueWith(task => _ = task.Exception, TaskContinuationOptions.OnlyOnFaulted);
    await context.CallAsync(hostEcho, new Value { Value_ = ByteString.CopyFromUtf8("release") });
    await cancelled.CancelAsync();
    try
    {
      await pending;
    }
    catch (OperationCanceledException) { }
    var automatic = context.CallAsync(hostPending, new Value());
    _ = automatic.ContinueWith(task => _ = task.Exception, TaskContinuationOptions.OnlyOnFaulted);
    return new Value { Value_ = ByteString.CopyFromUtf8("cancelled") };
  }

  private Task<Value> PeerCall(global::Harmos.Plugins.Context context, Value request) => request.Value_.ToStringUtf8() switch
  {
    "public" => context.CallAsync(peerPublic, new Value { Value_ = ByteString.CopyFromUtf8(context.Scope()) }),
    "protected" => context.CallAsync(peerProtected, request),
    "self" => context.CallAsync(ownProtected, request),
    _ => context.CallAsync(peerLocal, request),
  };

  private async Task<Value> Local(global::Harmos.Plugins.Context context, Value request)
  {
    if (request.Value_.ToStringUtf8() is "deadline" or "overall")
    {
      await context.Host.Time.Sleep(new Duration { Seconds = 3600 });
      return request;
    }
    if (request.Value_ == ByteString.CopyFromUtf8("cancel"))
    {
      await context.CallAsync(hostEcho, request);
      return new Value { Value_ = ByteString.CopyFromUtf8(await context.CancelledAsync()) };
    }
    if (request.Value_.ToStringUtf8() is not ("retry" or "retry-bound"))
      return request;
    if (context.Attempt == 1)
      throw new PluginFailureException(new Failure { Classification = Classification.Transient, Code = "retry", Message = "Retry" });
    return new Value { Value_ = ByteString.CopyFromUtf8(context.Attempt.ToString()) };
  }
  private Task<Value> LocalCall(global::Harmos.Plugins.Context context, Value request)
  {
    var policy = request.Value_.ToStringUtf8() switch
    {
      "retry" => new OperationPolicy { Retries = new RetryPolicy { TotalAttempts = 2, Initial = new Duration { Seconds = 1 }, Maximum = new Duration { Seconds = 4 }, DeterministicJitter = true } },
      "deadline" => new OperationPolicy { Timeout = new Duration { Seconds = 3 }, AttemptTimeout = new Duration { Seconds = 2 } },
      "cancel" => new OperationPolicy { CancellationTimeout = new Duration { Seconds = 1 } },
      "overall" => new OperationPolicy { Timeout = new Duration { Seconds = 2 }, CancellationTimeout = new Duration { Seconds = 1 } },
      "retry-bound" => new OperationPolicy { Timeout = new Duration { Seconds = 1 }, Retries = new RetryPolicy { TotalAttempts = 2, Initial = new Duration { Seconds = 3600 } } },
      _ => null
    };
    return context.CallAsync(local, request, policy);
  }

  private async Task<Value> Watch(global::Harmos.Plugins.Context context, Value request)
  {
    var value = new List<byte>();
    await foreach (var item in context.WatchAsync(hostWatch, request, 4096))
    {
      value.AddRange(item.Value_);
      if (request.Value_ == ByteString.CopyFromUtf8("cancel"))
        break;
    }
    return new Value { Value_ = ByteString.CopyFrom(value.ToArray()) };
  }

  private async Task<Value> Resources(global::Harmos.Plugins.Context context, Value request)
  {
    context.Set(request.Clone());
    await context.CallAsync(hostEcho, request);
    return context.Get<Value>();
  }

  private async Task<Value> CancelReason(global::Harmos.Plugins.Context context, Value request)
  {
    await context.CallAsync(hostEcho, request);
    return new Value { Value_ = ByteString.CopyFromUtf8(await context.CancelledAsync()) };
  }

  private async Task<Value> ClockCall(global::Harmos.Plugins.Context context, Value request)
  {
    var before = await context.Host.Time.Get();
    await context.Host.Time.Sleep(new Duration { Seconds = 3600 });
    var after = await context.Host.Time.Get();
    var bytes = new byte[sizeof(long)];
    BinaryPrimitives.WriteInt64LittleEndian(bytes, after.Seconds - before.Seconds);
    return new Value { Value_ = ByteString.CopyFrom(bytes) };
  }

  private Task<Value> Echo(global::Harmos.Plugins.Context context, Value request) => Task.FromResult(
      request.Value_ == ByteString.CopyFromUtf8("scope")
          ? new Value { Value_ = ByteString.CopyFromUtf8(context.Scope()) }
          : request);

  private async Task<Value> Children(global::Harmos.Plugins.Context context, Value request)
  {
    var first = context.CallAsync(hostEcho, new Value { Value_ = ByteString.CopyFrom([1]) });
    var second = context.CallAsync(hostEcho, new Value { Value_ = ByteString.CopyFrom([2]) });
    var replies = await Task.WhenAll(first, second);
    var bytes = replies.SelectMany(reply => reply.Value_).ToArray();
    return new Value { Value_ = ByteString.CopyFrom(bytes) };
  }

  private async Task<Value> Policy(global::Harmos.Plugins.Context context, Value request)
  {
    var first = await context.CallAsync(hostEcho, request, new OperationPolicy { Retries = new RetryPolicy { TotalAttempts = 3, Initial = new Duration { Seconds = 1 }, Maximum = new Duration { Seconds = 4 }, DeterministicJitter = true } });
    return await context.CallAsync(hostEcho, first);
  }

  private Task<Value> Cancel(global::Harmos.Plugins.Context context, Value request) => context.CallAsync(hostPending, request);

  private Task<Value> ChildFailure(global::Harmos.Plugins.Context context, Value request) => context.CallAsync(hostFail, request);

  private Task<Value> Failure(global::Harmos.Plugins.Context context, Value request) => throw new PluginFailureException(new Failure
  {
    Classification = Classification.Permanent,
    Code = "fixture_failure",
    Message = "Fixture Failed",
    Action = "Use Another Input",
    Operation = "conformance.failure",
    Plugin = "conformance",
    Diagnostics = "Fixture Diagnostics",
    Details = { ["fixture"] = "value" },
    Cause = new Failure
    {
      Classification = Classification.Transient,
      Code = "fixture_cause",
      Message = "Fixture Cause",
      Action = "Retry",
      Operation = "conformance.host.fail",
      Plugin = "host"
    }
  });

  private Task<Value> Sequence(global::Harmos.Plugins.Context context)
  {
    var bytes = new byte[sizeof(ulong)];
    BinaryPrimitives.WriteUInt64LittleEndian(bytes, context.Sequence);
    return Task.FromResult(new Value { Value_ = ByteString.CopyFrom(bytes) });
  }

  /// <summary>Each sample holds as many bytes as its sequence, so a grant can exceed the message size.</summary>
  private Task<Value> Oversized(global::Harmos.Plugins.Context context) =>
    Task.FromResult(new Value { Value_ = ByteString.CopyFrom(new byte[checked((int)context.Sequence)]) });

  private Task<Values> Sequences(global::Harmos.Plugins.Context context, Window window)
  {
    var columns = new Values();
    var bytes = new byte[sizeof(ulong)];
    for (ulong index = 0; index < window.Count; index++)
    {
      BinaryPrimitives.WriteUInt64LittleEndian(bytes, window.Sequence + index);
      columns.Value.Add(ByteString.CopyFrom(bytes));
    }
    return Task.FromResult(columns);
  }

  private async Task Device(global::Harmos.Plugins.Context context, Publisher<Value> publisher)
  {
    await publisher.PublishAsync(new Value { Value_ = ByteString.CopyFromUtf8("device") });
    await context.CancelledAsync();
  }
}
