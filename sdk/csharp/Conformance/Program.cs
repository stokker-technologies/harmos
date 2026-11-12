// SPDX-License-Identifier: Apache-2.0

using System.Buffers.Binary;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Harmos.Sdk;
using Harmos.V1;
using Graph = Harmos.Conformance.V1.Graph;
using Value = Harmos.Conformance.V1.Value;
using Plugin = Harmos.Sdk.Plugin;

var fixture = new ConformancePlugin();
await fixture.Build().ServeAsync(remote: args.Contains("--remote"));

internal sealed class ConformancePlugin
{
  private readonly Operation<Value, Value> hostEcho = new("conformance.host.echo", Visibility.Public);
  private readonly Operation<Value, Value> hostPending = new("conformance.host.pending", Visibility.Public);
  private readonly Operation<Value, Value> hostFail = new("conformance.host.fail", Visibility.Public);

  public Plugin Build() => new Plugin("conformance", "1.0.0")
      .Message<Harmos.Conformance.V1.PayloadOnly>()
      .Message<Harmos.Conformance.V1.PayloadOnly>()
      .Operation(new Operation<Value, Value>("conformance.echo", Visibility.Public), Echo)
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
      .Requires(hostEcho)
      .Requires(hostPending)
      .Requires(hostFail)
      .Requires(new Operation<Graph, Value>("conformance.host.schema", Visibility.Public))
      .Requires(Clock.Sleep)
      .Requires(Clock.Now)
      .Observer("conformance.ticks", new Duration { Seconds = 1 }, Tick)
      .Stream<Value>("conformance.device", Device)
      .Observer("conformance.settings", new Duration { Seconds = 1 }, Tick, intervalSetting: "harmos.conformance.v1.Graph.interval")
      .Section(new Graph { Interval = new Duration { Seconds = 2 } });

  private async Task<Value> ClockCall(Context context, Value request)
  {
    var before = await context.NowAsync();
    await context.SleepAsync(new Duration { Seconds = 3600 });
    var after = await context.NowAsync();
    var bytes = new byte[sizeof(long)];
    BinaryPrimitives.WriteInt64LittleEndian(bytes, after.Seconds - before.Seconds);
    return new Value { Value_ = ByteString.CopyFrom(bytes) };
  }

  private Task<Value> Echo(Context context, Value request) => Task.FromResult(
      request.Value_ == ByteString.CopyFromUtf8("scope")
          ? new Value { Value_ = ByteString.CopyFromUtf8(context.Scope()) }
          : request);

  private async Task<Value> Children(Context context, Value request)
  {
    var first = context.CallAsync(hostEcho, new Value { Value_ = ByteString.CopyFrom([1]) });
    var second = context.CallAsync(hostEcho, new Value { Value_ = ByteString.CopyFrom([2]) });
    var replies = await Task.WhenAll(first, second);
    var bytes = replies.SelectMany(reply => reply.Value_).ToArray();
    return new Value { Value_ = ByteString.CopyFrom(bytes) };
  }

  private async Task<Value> Policy(Context context, Value request)
  {
    var first = await context.CallAsync(hostEcho, request, new OperationPolicy { Retries = new RetryPolicy { TotalAttempts = 3, Initial = new Duration { Seconds = 1 } } });
    return await context.CallAsync(hostEcho, first);
  }

  private Task<Value> Cancel(Context context, Value request) => context.CallAsync(hostPending, request);

  private Task<Value> ChildFailure(Context context, Value request) => context.CallAsync(hostFail, request);

  private Task<Value> Failure(Context context, Value request) => throw new PluginFailureException(new Failure
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

  private Task<Value> Tick(Context context)
  {
    var bytes = new byte[sizeof(ulong)];
    BinaryPrimitives.WriteUInt64LittleEndian(bytes, context.Sequence);
    return Task.FromResult(new Value { Value_ = ByteString.CopyFrom(bytes) });
  }

  private async Task Device(Context context, Publisher<Value> publisher)
  {
    await publisher.PublishAsync(new Value { Value_ = ByteString.CopyFromUtf8("device") });
    await context.CancelledAsync();
  }
}
