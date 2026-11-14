// SPDX-License-Identifier: Apache-2.0

using System.Collections.Generic;
using System.Linq;
using System.Threading.Channels;
using System.Threading.Tasks;
using System.Threading;
using System;
using Google.Protobuf.WellKnownTypes;
using Google.Protobuf;
using Grpc.Core;
using Harmos.Internal.Plugin;
using Harmos.Clients;
using Harmos.Plugins;
using Harmos;
using Microsoft.Extensions.Hosting;
using Xunit;
using SdkContext = Harmos.Plugins.Context;
using SdkPlugin = Harmos.Plugins.Plugin;
using Status = Grpc.Core.Status;

namespace Harmos.Tests.Unit;

public sealed class SamplesCalls
{
  private static readonly Operation<StringValue, StringValue> Echo = new("fixture.echo", Visibility.Public);
  private static readonly Operation<StringValue, StringValue> Watch = new("fixture.watch", Visibility.Public, watch: true);

  [Fact]
  public async Task ReverseRepliesCorrelateAcrossTicksAndIndependentStreams()
  {
    // Open Two Independent Demand Calls.
    var service = Service(async context =>
    {
      var first = context.CallAsync(Echo, new StringValue { Value = "first" });
      var second = context.CallAsync(Echo, new StringValue { Value = "second" });
      var replies = await Task.WhenAll(first, second);
      return new StringValue { Value = string.Concat(replies.Select(reply => reply.Value)) };
    });
    var firstCall = new Call();
    var secondCall = new Call();
    var firstServing = service.Samples(firstCall, firstCall, firstCall.Context);
    var secondServing = service.Samples(secondCall, secondCall, secondCall.Context);
    ulong previous = 0;
    for (var tick = 1; tick <= 2; tick++)
    {
      // Reply in Reverse Order and Preserve the Tick's Row and Timestamp.
      await firstCall.Tick(tick);
      var first = (await firstCall.Read()).Child;
      var second = (await firstCall.Read()).Child;
      Assert.True(first.Call > previous);
      Assert.True(second.Call > first.Call);
      await firstCall.Reply(second.Call, new StringValue { Value = "second" });
      await firstCall.Reply(first.Call, new StringValue { Value = "first" });
      var answer = await firstCall.Read();
      var sample = Assert.Single(answer.Samples);
      Assert.Equal(InstanceSamplesResponse.FrameOneofCase.None, answer.FrameCase);
      Assert.Equal("firstsecond", StringValue.Parser.ParseFrom(sample.Row).Value);
      Assert.Equal((ulong)tick, sample.Sequence);
      Assert.Equal(tick, sample.At.Seconds);
      previous = second.Call;
    }
    await secondCall.Tick(3);
    var independentFirst = (await secondCall.Read()).Child;
    var independentSecond = (await secondCall.Read()).Child;
    Assert.Equal(1UL, independentFirst.Call);
    await secondCall.Reply(independentSecond.Call, new StringValue { Value = "second" });
    await secondCall.Reply(independentFirst.Call, new StringValue { Value = "first" });
    Assert.Single((await secondCall.Read()).Samples);
    firstCall.Close();
    secondCall.Close();
    await Task.WhenAll(firstServing, secondServing);
  }

  [Theory]
  [InlineData(null)]
  [InlineData("fixture")]
  public async Task ObserverReadsClockSettingsAndDemandBoundedWatch(string? name)
  {
    var expected = new SettingsListResponse
    {
      Declarations = { new SettingDeclaration { Name = "prefix", Default = new SettingValue { Text = "default" } } },
      Value = new SettingValues { Values = { ["prefix"] = new SettingValue { Text = "tick" } } },
      Revision = ByteString.CopyFromUtf8("settings-revision"),
      ResolvedAt = new ReadCut { Positions = { new Position { History = new History { Store = "fixture", Epoch = "epoch" }, Sequence = 7 } } }
    };
    var service = Service(async context =>
    {
      var now = await context.Host.Time.Get();
      var settings = await context.Host.Plugins.Settings.List(name: name);
      Assert.Equal(expected, settings);
      var result = settings.Value.Values["prefix"].Text + now.Seconds;
      await foreach (var item in context.WatchAsync(Watch, new StringValue(), 4096))
      {
        result += item.Value;
        break;
      }
      return new StringValue { Value = result };
    });
    var call = new Call();
    var serving = service.Samples(call, call, call.Context);
    await call.Tick(7);
    var clock = (await call.Read()).Child;
    Assert.Equal(HostContracts.TimeGet.Contract.Id, clock.Operation);
    await call.Reply(clock.Call, new TimeGetResponse { Now = new Timestamp { Seconds = 7 } });
    var settings = (await call.Read()).Child;
    Assert.Equal(HostContracts.PluginsSettingsList.Contract.Id, settings.Operation);
    var selector = SettingsListRequest.Parser.ParseFrom(settings.Payload);
    Assert.Equal(name is not null, selector.HasName);
    Assert.Equal(name ?? "", selector.Name);
    await call.Reply(settings.Call, expected);
    var watch = (await call.Read()).Child;
    Assert.True(watch.Watch);
    Assert.Equal(watch.Call, (await call.Read()).ChildDemand.Call);
    await call.Send(new InstanceSamplesRequest { Child = new ChildResponse { Call = watch.Call, Item = new StringValue { Value = ":item" }.ToByteString() } });
    Assert.Equal(watch.Call, (await call.Read()).ChildCancel.Call);
    Assert.Equal("tick7:item", StringValue.Parser.ParseFrom(Assert.Single((await call.Read()).Samples).Row).Value);
    call.Close();
    await serving;
  }

  [Fact]
  public async Task ChildFailurePreservesEveryFieldForTheObserver()
  {
    var failure = new Failure
    {
      Classification = Classification.Refused,
      Code = "fixture",
      Message = "fixture",
      Action = "Retry",
      Operation = Echo.Contract.Id,
      Plugin = "fixture",
      Diagnostics = "details",
      Details = { ["key"] = "value" },
      Cause = new Failure { Code = "cause", Classification = Classification.Permanent }
    };
    var service = Service(async context =>
    {
      var error = await Assert.ThrowsAsync<PluginFailureException>(() => context.CallAsync(Echo, new StringValue()));
      Assert.Equal(failure, error.Failure);
      return new StringValue { Value = "handled" };
    });
    var call = new Call();
    var serving = service.Samples(call, call, call.Context);
    await call.Tick(1);
    var child = (await call.Read()).Child;
    await call.Send(new InstanceSamplesRequest { Child = new ChildResponse { Call = child.Call, Failure = failure } });
    Assert.Single((await call.Read()).Samples);
    call.Close();
    await serving;
  }

  [Fact]
  public async Task TickAnswerCancelsOpenChildrenAndRefusesLateReplies()
  {
    Task<StringValue>? pending = null;
    var service = Service(context =>
    {
      pending = context.CallAsync(Echo, new StringValue());
      return Task.FromResult(new StringValue());
    });
    var call = new Call();
    var serving = service.Samples(call, call, call.Context);
    await call.Tick(1);
    var child = (await call.Read()).Child;
    Assert.Equal(child.Call, (await call.Read()).ChildCancel.Call);
    Assert.Single((await call.Read()).Samples);
    await Assert.ThrowsAsync<PluginFailureException>(async () => await pending!);
    await call.Reply(child.Call, new StringValue());
    var refusal = await Assert.ThrowsAsync<RpcException>(() => serving);
    Assert.Equal(StatusCode.InvalidArgument, refusal.StatusCode);
  }

  [Fact]
  public async Task StreamEndCancelsTheUnansweredTick()
  {
    Task<StringValue>? pending = null;
    var service = Service(context => pending = context.CallAsync(Echo, new StringValue()));
    var call = new Call();
    var serving = service.Samples(call, call, call.Context);
    await call.Tick(1);
    Assert.NotNull((await call.Read()).Child);
    call.Close();
    await serving;
    await Assert.ThrowsAsync<RpcException>(async () => await pending!);
  }

  [Theory]
  [InlineData(false)]
  [InlineData(true)]
  public async Task ChildTrafficBeforeATickOrOnADeviceStreamIsRefused(bool device)
  {
    var service = Service(context => Task.FromResult(new StringValue()), device);
    var call = new Call();
    var serving = service.Samples(call, call, call.Context);
    await call.Reply(1, new StringValue());
    var refusal = await Assert.ThrowsAsync<RpcException>(() => serving);
    Assert.Equal(StatusCode.InvalidArgument, refusal.StatusCode);
  }

  private static PluginService Service(Func<SdkContext, Task<StringValue>> observer, bool device = false)
  {
    var plugin = new SdkPlugin("fixture", "1.0.0").Requires(Echo).Requires(Watch).Requires(HostOperation.TimeGet).Requires(HostOperation.PluginsSettingsList);
    var publication = device
        ? new Publication(null, async (context, rows) => { await context.CancelledAsync(); })
        : new Publication(async context => (await observer(context)).ToByteString(), null);
    return new PluginService(new InstanceGetResponse { Id = "fixture", Declarations = plugin.Declarations() },
        [], new Dictionary<string, Publication> { ["fixture"] = publication }, [], true, new Lifetime());
  }

  private sealed class Call : IAsyncStreamReader<InstanceSamplesRequest>, IServerStreamWriter<InstanceSamplesResponse>
  {
    private readonly Channel<InstanceSamplesRequest> input = Channel.CreateUnbounded<InstanceSamplesRequest>();
    private readonly Channel<InstanceSamplesResponse> output = Channel.CreateUnbounded<InstanceSamplesResponse>();
    public Call() => input.Writer.TryWrite(new InstanceSamplesRequest { Open = new SamplesSelection { Stream = "fixture", Scope = "opaque" } });
    public ServerCallContext Context { get; } = new ServerContext();
    public InstanceSamplesRequest Current { get; private set; } = new();
    public WriteOptions? WriteOptions
    {
      get; set;
    }
    public async Task<bool> MoveNext(CancellationToken cancellationToken)
    {
      if (!await input.Reader.WaitToReadAsync(cancellationToken))
        return false;
      Current = await input.Reader.ReadAsync(cancellationToken);
      return true;
    }
    public Task WriteAsync(InstanceSamplesResponse message) => output.Writer.WriteAsync(message).AsTask();
    public ValueTask Send(InstanceSamplesRequest message) => input.Writer.WriteAsync(message);
    public ValueTask Tick(long seconds) => Send(new InstanceSamplesRequest { Tick = new Tick { At = new Timestamp { Seconds = seconds } } });
    public ValueTask Reply(ulong id, IMessage value) => Send(new InstanceSamplesRequest { Child = new ChildResponse { Call = id, Payload = value.ToByteString() } });
    public ValueTask<InstanceSamplesResponse> Read() => output.Reader.ReadAsync();
    public void Close() => input.Writer.TryComplete();
  }

  private sealed class ServerContext : ServerCallContext
  {
    protected override string MethodCore => "Samples";
    protected override string HostCore => "fixture";
    protected override string PeerCore => "fixture";
    protected override DateTime DeadlineCore => DateTime.MaxValue;
    protected override Metadata RequestHeadersCore { get; } = new();
    protected override CancellationToken CancellationTokenCore => CancellationToken.None;
    protected override Metadata ResponseTrailersCore { get; } = new();
    protected override Status StatusCore
    {
      get; set;
    }
    protected override WriteOptions? WriteOptionsCore
    {
      get; set;
    }
    protected override AuthContext AuthContextCore { get; } = new("fixture", new Dictionary<string, List<AuthProperty>>());
    protected override ContextPropagationToken CreatePropagationTokenCore(ContextPropagationOptions? options) => throw new NotSupportedException();
    protected override Task WriteResponseHeadersAsyncCore(Metadata responseHeaders) => Task.CompletedTask;
  }

  private sealed class Lifetime : IHostApplicationLifetime
  {
    public CancellationToken ApplicationStarted => CancellationToken.None;
    public CancellationToken ApplicationStopping => CancellationToken.None;
    public CancellationToken ApplicationStopped => CancellationToken.None;
    public void StopApplication()
    {
    }
  }
}
