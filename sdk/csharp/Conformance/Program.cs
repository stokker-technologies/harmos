// SPDX-License-Identifier: Apache-2.0

using System.Buffers.Binary;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Harmos.Sdk;
using Harmos.V1;
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
      .Operation(new Operation<Value, Value>("conformance.echo", Visibility.Public), Echo)
      .Operation(new Operation<Value, Value>("conformance.children", Visibility.Public), Children)
      .Operation(new Operation<Value, Value>("conformance.cancel", Visibility.Public), Cancel)
      .Operation(new Operation<Value, Value>("conformance.failure", Visibility.Public), Failure)
      .Operation(new Operation<Value, Value>("conformance.child_failure", Visibility.Public), ChildFailure)
      .Requires(hostEcho)
      .Requires(hostPending)
      .Requires(hostFail)
      .Observer("conformance.ticks", new Duration { Seconds = 1 }, Tick)
      .Stream<Value>("conformance.device", Device);

  private Task<Value> Echo(Context context, Value request) => Task.FromResult(request);

  private async Task<Value> Children(Context context, Value request)
  {
    var first = context.CallAsync(hostEcho, new Value { Value_ = ByteString.CopyFrom([1]) });
    var second = context.CallAsync(hostEcho, new Value { Value_ = ByteString.CopyFrom([2]) });
    var replies = await Task.WhenAll(first, second);
    var bytes = replies.SelectMany(reply => reply.Value_).ToArray();
    return new Value { Value_ = ByteString.CopyFrom(bytes) };
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
