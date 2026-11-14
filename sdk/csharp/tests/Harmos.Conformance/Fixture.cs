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
using Harmos.Plugins;
using Harmos;
using Graph = Harmos.Conformance.Graph;
using SdkContext = global::Harmos.Plugins.Context;
using SdkPlugin = global::Harmos.Plugins.Plugin;
using Value = Harmos.Conformance.Value;

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

  public SdkPlugin Build() => new SdkPlugin("conformance", "1.0.0")
      .Message<Harmos.Conformance.PayloadOnly>()
      .Message<Harmos.Conformance.PayloadOnly>()
      .Operation(local, Local)
      .Operation(new Operation<Value, Value>("conformance.local_call", Visibility.Public), LocalCall)
      .Operation(new Operation<Value, Value>("conformance.step.0", Visibility.Public), Echo)
      .Operation(new Operation<Value, Value>("conformance.children", Visibility.Public), Children)
      .Operation(new Operation<Value, Value>("conformance.cancel", Visibility.Public), Cancel)
      .Operation(new Operation<Value, Value>("conformance.failure", Visibility.Public), Failure)
      .Operation(new Operation<Value, Value>("conformance.child_failure", Visibility.Public), ChildFailure)
      .Operation(new Operation<Value, Value>("conformance.policy", Visibility.Public), Policy)
      .Operation(new Operation<Graph, Graph.Types.Nested>("conformance.schema", Visibility.Public),
          (context, request) => Task.FromResult(new Graph.Types.Nested { Count = request.Items.Count, Next = request }))
      .Operation(new Operation<Graph.Types.Nested, Graph>("conformance.nested", Visibility.Public),
          (context, request) => Task.FromResult(request.Next ?? new Graph()))
      .Operation(new Operation<Graph, Value>("conformance.projected", Visibility.Public),
          (context, request) => Task.FromResult(request.Tail?.Value ?? new Value()))
      .Operation(new Operation<Value, Value>("conformance.clock", Visibility.Public), ClockCall)
      .Operation(new Operation<Value, Value>("example.plugin.v1.Lifecycle/Initialize", Visibility.Protected), Echo)
      .Operation(new Operation<Value, Value>("example.plugin.v1.Steps/Execute", Visibility.Public, replaySafe: true), Resources)
      .Operation(new Operation<Value, Value>("conformance.cancel_reason", Visibility.Public), CancelReason)
      .Operation(new Operation<Value, Value>("conformance.watch", Visibility.Public), Watch)
      .Operation(new Operation<Value, Value>("conformance.peer_call", Visibility.Public), PeerCall)
      .Operation(new Operation<Value, Value>("conformance.settings_read", Visibility.Public), ReadSettings)
      .Operation(new Operation<Value, Value>("conformance.discover", Visibility.Public), Discover)
      .Requires(peerPublic).Requires(peerProtected).Requires(peerLocal).Requires(ownProtected)
      .Requires(hostWatch)
      .Requires(hostEcho)
      .Requires(hostPending)
      .Requires(hostFail)
      .Requires(new Operation<Graph, Value>("conformance.host.schema", Visibility.Public))
      .Requires(HostOperation.TimeSleep)
      .Requires(HostOperation.TimeGet)
      .Requires(HostOperation.SettingsList)
      .Requires(HostOperation.OperationsList)
      .Requires(HostOperation.OperationsGet)
      .Observer("conformance.ticks", new Duration { Seconds = 1 }, Tick)
      .Stream<Value>("conformance.device", Device)
      .Observer("conformance.settings", new Duration { Seconds = 1 }, Tick, intervalSetting: "interval")
      .Observer("conformance.tick_children", new Duration { Seconds = 1 }, context => Children(context, new Value()))
      .Observer("conformance.tick_context", new Duration { Seconds = 1 }, TickContext)
      .Observer("conformance.tick_failure", new Duration { Seconds = 1 }, TickFailure)
      .Observer("conformance.tick_watch", new Duration { Seconds = 1 }, context => Watch(context, new Value { Value_ = ByteString.CopyFromUtf8("fixture") }))
      .Observer("conformance.tick_cancel", new Duration { Seconds = 1 }, TickCancel)
      .Section(new Graph { Interval = new Duration { Seconds = 2 } })
      .Setting("count", new SettingValue { Integer = 3 }, "count", "Conformance count")
      .Setting("enabled", new SettingValue { Flag = false }, "enabled", "Conformance enabled")
      .Setting("gain", new SettingValue { Number = 1.5 }, "gain", "Conformance gain")
      .Setting("interval", new SettingValue { Duration = new Duration { Seconds = 2 } }, "interval", "Conformance interval")
      .Setting("label", new SettingValue { Text = "default" }, "label", "Conformance label");

  private static async Task<Value> ReadSettings(SdkContext context, Value request)
  {
    var settings = await context.Host.Settings.List();
    return new Value { Value_ = settings.Value.ToByteString() };
  }

  private async Task<Value> Discover(SdkContext context, Value request)
  {
    var operations = await context.Host.Operations.List();
    var id = operations[0].Contract.Id;
    var operation = await context.Host.Operations.Get(id);
    return new Value { Value_ = ByteString.CopyFromUtf8(operation.Contract.Id) };
  }

  private async Task<Value> TickContext(SdkContext context)
  {
    var echo = await context.CallAsync(hostEcho, new Value { Value_ = ByteString.CopyFromUtf8("fixture") });
    var clock = new TimeGetResponse { Now = await context.Host.Time.Get() };
    var settings = await context.Host.Settings.List();
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

  private async Task<Value> TickFailure(SdkContext context)
  {
    try
    {
      return await context.CallAsync(hostFail, new Value());
    }
    catch (PluginFailureException error) { return new Value { Value_ = error.Failure.ToByteString() }; }
  }

  private async Task<Value> TickCancel(SdkContext context)
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

  private Task<Value> PeerCall(SdkContext context, Value request) => request.Value_.ToStringUtf8() switch
  {
    "public" => context.CallAsync(peerPublic, new Value { Value_ = ByteString.CopyFromUtf8(context.Scope()) }),
    "protected" => context.CallAsync(peerProtected, request),
    "self" => context.CallAsync(ownProtected, request),
    _ => context.CallAsync(peerLocal, request),
  };

  private async Task<Value> Local(SdkContext context, Value request)
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
  private Task<Value> LocalCall(SdkContext context, Value request)
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

  private async Task<Value> Watch(SdkContext context, Value request)
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

  private async Task<Value> Resources(SdkContext context, Value request)
  {
    context.Set(request.Clone());
    await context.CallAsync(hostEcho, request);
    return context.Get<Value>();
  }

  private async Task<Value> CancelReason(SdkContext context, Value request)
  {
    await context.CallAsync(hostEcho, request);
    return new Value { Value_ = ByteString.CopyFromUtf8(await context.CancelledAsync()) };
  }

  private async Task<Value> ClockCall(SdkContext context, Value request)
  {
    var before = await context.Host.Time.Get();
    await context.Host.Time.Sleep(new Duration { Seconds = 3600 });
    var after = await context.Host.Time.Get();
    var bytes = new byte[sizeof(long)];
    BinaryPrimitives.WriteInt64LittleEndian(bytes, after.Seconds - before.Seconds);
    return new Value { Value_ = ByteString.CopyFrom(bytes) };
  }

  private Task<Value> Echo(SdkContext context, Value request) => Task.FromResult(
      request.Value_ == ByteString.CopyFromUtf8("scope")
          ? new Value { Value_ = ByteString.CopyFromUtf8(context.Scope()) }
          : request);

  private async Task<Value> Children(SdkContext context, Value request)
  {
    var first = context.CallAsync(hostEcho, new Value { Value_ = ByteString.CopyFrom([1]) });
    var second = context.CallAsync(hostEcho, new Value { Value_ = ByteString.CopyFrom([2]) });
    var replies = await Task.WhenAll(first, second);
    var bytes = replies.SelectMany(reply => reply.Value_).ToArray();
    return new Value { Value_ = ByteString.CopyFrom(bytes) };
  }

  private async Task<Value> Policy(SdkContext context, Value request)
  {
    var first = await context.CallAsync(hostEcho, request, new OperationPolicy { Retries = new RetryPolicy { TotalAttempts = 3, Initial = new Duration { Seconds = 1 }, Maximum = new Duration { Seconds = 4 }, DeterministicJitter = true } });
    return await context.CallAsync(hostEcho, first);
  }

  private Task<Value> Cancel(SdkContext context, Value request) => context.CallAsync(hostPending, request);

  private Task<Value> ChildFailure(SdkContext context, Value request) => context.CallAsync(hostFail, request);

  private Task<Value> Failure(SdkContext context, Value request) => throw new PluginFailureException(new Failure
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

  private Task<Value> Tick(SdkContext context)
  {
    var bytes = new byte[sizeof(ulong)];
    BinaryPrimitives.WriteUInt64LittleEndian(bytes, context.Sequence);
    return Task.FromResult(new Value { Value_ = ByteString.CopyFrom(bytes) });
  }

  private async Task Device(SdkContext context, Publisher<Value> publisher)
  {
    await publisher.PublishAsync(new Value { Value_ = ByteString.CopyFromUtf8("device") });
    await context.CancelledAsync();
  }
}
