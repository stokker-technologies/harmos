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
using Harmos.Tests.Columns;
using Harmos;
using Microsoft.Extensions.Hosting;
using Xunit;
using SdkContext = Harmos.Plugins.Context;
using SdkPlugin = Harmos.Plugins.Plugin;
using Status = Grpc.Core.Status;
using Window = Harmos.Plugins.Window;
using WireWindow = Harmos.Internal.Plugin.Window;

namespace Harmos.Tests.Unit;

public sealed class SamplesCalls
{
  private static readonly Operation<StringValue, StringValue> Echo = new("fixture.echo", Visibility.Public);
  private static readonly Operation<StringValue, StringValue> Watch = new("fixture.watch", Visibility.Public, watch: true);

  [Fact]
  public async Task ReverseRepliesCorrelateAcrossWindowsAndIndependentStreams()
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
    foreach (var (sequence, count) in new[] { (1UL, 2UL), (3UL, 1UL) })
    {
      // Reply in Reverse Order for Every Sample; IDs Keep Growing Across Samples and Windows.
      await firstCall.Grant(sequence, count);
      for (ulong sample = 0; sample < count; sample++)
      {
        var first = (await firstCall.Read()).Child;
        var second = (await firstCall.Read()).Child;
        Assert.True(first.Call > previous);
        Assert.True(second.Call > first.Call);
        await firstCall.Reply(second.Call, new StringValue { Value = "second" });
        await firstCall.Reply(first.Call, new StringValue { Value = "first" });
        previous = second.Call;
      }
      var batch = await firstCall.ReadBatch();
      Assert.Equal((0UL, count, true), (batch.Offset, batch.Count, batch.Last));
      Assert.Equal(Enumerable.Repeat("firstsecond", (int)count), Labels.Parser.ParseFrom(batch.Columns).Value);
    }
    await secondCall.Grant(1);
    var independentFirst = (await secondCall.Read()).Child;
    var independentSecond = (await secondCall.Read()).Child;
    Assert.Equal(1UL, independentFirst.Call);
    await secondCall.Reply(independentSecond.Call, new StringValue { Value = "second" });
    await secondCall.Reply(independentFirst.Call, new StringValue { Value = "first" });
    Assert.Equal(1UL, (await secondCall.ReadBatch()).Count);
    firstCall.Close();
    secondCall.Close();
    await Task.WhenAll(firstServing, secondServing);
  }

  [Fact]
  public async Task OneSampleObserverRunsOncePerSampleWithItsSequenceAndTime()
  {
    var service = Service(context => Task.FromResult(new StringValue { Value = $"{context.Sequence}@{context.At!.Seconds}.{context.At.Nanos}" }));
    var call = new Call();
    var serving = service.Samples(call, call, call.Context);
    await call.Send(new InstanceSamplesRequest
    {
      Window = new WireWindow
      {
        Start = new Timestamp { Seconds = 10, Nanos = 500_000_000 },
        Period = new Duration { Nanos = 750_000_000 },
        Count = 3,
        Sequence = 5
      }
    });
    var batch = await call.ReadBatch();
    Assert.Equal((0UL, 3UL, true), (batch.Offset, batch.Count, batch.Last));
    Assert.Equal(["5@10.500000000", "6@11.250000000", "7@12.0"], Labels.Parser.ParseFrom(batch.Columns).Value);
    call.Close();
    await serving;
  }

  [Theory]
  [InlineData(null)]
  [InlineData("fixture")]
  public async Task ObserverReadsClockSettingsAndDemandBoundedWatch(string? name)
  {
    var expected = new SettingsListResponse
    {
      Declarations = { new SettingDeclaration { Name = "prefix", Default = new SettingValue { Text = "default" } } },
      Value = new SettingValues { Values = { ["prefix"] = new SettingValue { Text = "window" } } },
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
    await call.Grant(1);
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
    Assert.Equal(["window7:item"], Labels.Parser.ParseFrom((await call.ReadBatch()).Columns).Value);
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
    await call.Grant(1);
    var child = (await call.Read()).Child;
    await call.Send(new InstanceSamplesRequest { Child = new ChildResponse { Call = child.Call, Failure = failure } });
    Assert.Equal(["handled"], Labels.Parser.ParseFrom((await call.ReadBatch()).Columns).Value);
    call.Close();
    await serving;
  }

  [Fact]
  public async Task WindowAnswerCancelsOpenChildrenAndRefusesLateReplies()
  {
    var pending = new List<Task<StringValue>>();
    var service = Service(context =>
    {
      pending.Add(context.CallAsync(Echo, new StringValue()));
      return Task.FromResult(new StringValue());
    });
    var call = new Call();
    var serving = service.Samples(call, call, call.Context);

    // Children Left Open by Earlier Samples Stay Open Until the Whole Window Is Answered.
    await call.Grant(1, 2);
    var first = (await call.Read()).Child;
    var second = (await call.Read()).Child;
    Assert.Equal(first.Call, (await call.Read()).ChildCancel.Call);
    Assert.Equal(second.Call, (await call.Read()).ChildCancel.Call);
    Assert.Equal(2UL, (await call.ReadBatch()).Count);
    foreach (var child in pending)
      await Assert.ThrowsAsync<PluginFailureException>(async () => await child);
    await call.Reply(first.Call, new StringValue());
    var refusal = await Assert.ThrowsAsync<RpcException>(() => serving);
    Assert.Equal(StatusCode.InvalidArgument, refusal.StatusCode);
  }

  [Fact]
  public async Task StreamEndCancelsTheUnansweredWindow()
  {
    Task<StringValue>? pending = null;
    var service = Service(context => pending = context.CallAsync(Echo, new StringValue()));
    var call = new Call();
    var serving = service.Samples(call, call, call.Context);
    await call.Grant(1);
    Assert.NotNull((await call.Read()).Child);
    call.Close();
    await serving;
    await Assert.ThrowsAsync<RpcException>(async () => await pending!);
  }

  [Theory]
  [InlineData(false)]
  [InlineData(true)]
  public async Task ChildTrafficBeforeAWindowOrOnADeviceStreamIsRefused(bool device)
  {
    var service = Service(context => Task.FromResult(new StringValue()), device);
    var call = new Call();
    var serving = service.Samples(call, call, call.Context);
    await call.Reply(1, new StringValue());
    var refusal = await Assert.ThrowsAsync<RpcException>(() => serving);
    Assert.Equal(StatusCode.InvalidArgument, refusal.StatusCode);
  }

  [Theory]
  [InlineData("missing start")]
  [InlineData("start before range")]
  [InlineData("start nanos")]
  [InlineData("missing period")]
  [InlineData("zero period")]
  [InlineData("negative period")]
  [InlineData("zero count")]
  [InlineData("zero sequence")]
  [InlineData("last sample past range")]
  [InlineData("repeated open")]
  [InlineData("device window")]
  [InlineData("unanswered window")]
  public async Task InvalidWindowsAreRefused(string invalid)
  {
    var service = Service(context => context.CallAsync(Echo, new StringValue()), invalid == "device window");
    var call = new Call();
    var serving = service.Samples(call, call, call.Context);
    var window = new WireWindow { Start = new Timestamp { Seconds = 1 }, Period = new Duration { Seconds = 1 }, Count = 1, Sequence = 1 };
    switch (invalid)
    {
      case "missing start":
        window.Start = null;
        break;
      case "start before range":
        window.Start.Seconds = -62_135_596_801;
        break;
      case "start nanos":
        window.Start.Nanos = 1_000_000_000;
        break;
      case "missing period":
        window.Period = null;
        break;
      case "zero period":
        window.Period.Seconds = 0;
        break;
      case "negative period":
        window.Period.Seconds = -1;
        break;
      case "zero count":
        window.Count = 0;
        break;
      case "zero sequence":
        window.Sequence = 0;
        break;
      case "last sample past range":
        window.Start.Seconds = 253_402_300_798;
        window.Count = 3;
        break;
      case "repeated open":
        await call.Send(new InstanceSamplesRequest { Open = new SamplesSelection { Stream = "fixture" } });
        break;
      case "unanswered window":
        await call.Send(new InstanceSamplesRequest { Window = window.Clone() });
        Assert.NotNull((await call.Read()).Child);
        break;
    }
    await call.Send(new InstanceSamplesRequest { Window = window });
    var refusal = await Assert.ThrowsAsync<RpcException>(() => serving);
    Assert.Equal(StatusCode.InvalidArgument, refusal.StatusCode);
  }

  [Fact]
  public async Task BothObserverFormsEncodeOneColumnPerFieldInSampleOrder()
  {
    var rows = new[] { new Reading { Level = 1.5, Count = -2, Label = "a", Tag = new Tag { Name = "x" } }, new Reading() };
    var columns = new ReadingColumns { Level = { 1.5, 0 }, Count = { -2, 0 }, Label = { "a", "" }, Tag = { new Tag { Name = "x" }, new Tag() } };
    byte[] expected =
    [
      0x0A, 0x10, 0, 0, 0, 0, 0, 0, 0xF8, 0x3F, 0, 0, 0, 0, 0, 0, 0, 0,
      0x12, 0x0B, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x00,
      0x1A, 0x01, 0x61, 0x1A, 0x00,
      0x22, 0x03, 0x0A, 0x01, 0x78, 0x22, 0x00
    ];
    var plugins = new[]
    {
      new SdkPlugin("fixture", "1.0.0").Observer("fixture", new Duration { Seconds = 1 },
          context => Task.FromResult(rows[context.Sequence - 1])),
      new SdkPlugin("fixture", "1.0.0").WindowObserver<Reading, ReadingColumns>("fixture", new Duration { Seconds = 1 },
          (context, window) => Task.FromResult(columns))
    };
    foreach (var plugin in plugins)
    {
      var call = new Call();
      var serving = Serve(plugin).Samples(call, call, call.Context);
      await call.Grant(1, 2);
      var batch = await call.ReadBatch();
      Assert.Equal((0UL, 2UL, true), (batch.Offset, batch.Count, batch.Last));
      Assert.Equal(expected, batch.Columns.ToByteArray());
      Assert.Equal(columns, ReadingColumns.Parser.ParseFrom(batch.Columns));
      call.Close();
      await serving;
    }
  }

  [Fact]
  public async Task WindowPartsSplitAtSampleBoundariesWithinTheMessageLimit()
  {
    const ulong limit = 40;
    var plugin = new SdkPlugin("fixture", "1.0.0").Observer("fixture", new Duration { Seconds = 1 },
        context => Task.FromResult(new StringValue { Value = $"sample{context.Sequence}" }));
    var call = new Call();
    var serving = Serve(plugin, limit).Samples(call, call, call.Context);
    await call.Grant(1, 10);
    var parts = new List<SampleBatch>();
    var values = new List<string>();
    do
    {
      var response = await call.Read();
      Assert.True((ulong)response.CalculateSize() <= limit);
      parts.Add(response.Batch);
      values.AddRange(Labels.Parser.ParseFrom(response.Batch.Columns).Value);
    } while (!parts[^1].Last);
    Assert.True(parts.Count > 1);
    Assert.All(parts, part => Assert.True(part.Count >= 1));
    Assert.Equal(parts.Select(part => part.Offset), parts.Select((part, index) => parts.Take(index).Aggregate(0UL, (sum, earlier) => sum + earlier.Count)));
    Assert.Equal(10UL, parts.Aggregate(0UL, (sum, part) => sum + part.Count));
    Assert.Equal(Enumerable.Range(1, 10).Select(sequence => $"sample{sequence}"), values);
    call.Close();
    await serving;
  }

  [Fact]
  public async Task SampleLargerThanTheMessageLimitFailsTheWindow()
  {
    const ulong limit = 24;
    var plugin = new SdkPlugin("fixture", "1.0.0").Observer("fixture", new Duration { Seconds = 1 },
        context => Task.FromResult(new StringValue { Value = new string('x', 32) }));
    var call = new Call();
    var serving = Serve(plugin, limit).Samples(call, call, call.Context);
    await call.Grant(1);
    var refusal = await Assert.ThrowsAsync<RpcException>(() => serving);
    Assert.Equal(StatusCode.ResourceExhausted, refusal.StatusCode);
    Assert.Contains(StringValue.Descriptor.FullName, refusal.Status.Detail);
    Assert.Contains(limit.ToString(), refusal.Status.Detail);
  }

  [Fact]
  public async Task WindowObserverColumnsMustHoldOneValuePerSample()
  {
    var plugin = new SdkPlugin("fixture", "1.0.0").WindowObserver<Reading, ReadingColumns>("fixture", new Duration { Seconds = 1 },
        (context, window) => Task.FromResult(new ReadingColumns { Level = { 1 }, Count = { 1 }, Label = { "" }, Tag = { new Tag() } }));
    var call = new Call();
    var serving = Serve(plugin).Samples(call, call, call.Context);
    await call.Grant(1, 2);
    var refusal = await Assert.ThrowsAsync<RpcException>(() => serving);
    Assert.Equal(StatusCode.Internal, refusal.StatusCode);
  }

  private static PluginService Service(Func<SdkContext, Task<StringValue>> observer, bool device = false)
  {
    var plugin = new SdkPlugin("fixture", "1.0.0").Requires(Echo).Requires(Watch).Requires(HostOperation.TimeGet).Requires(HostOperation.PluginsSettingsList);
    plugin = device
        ? plugin.Stream<StringValue>("fixture", async (context, publisher) => { await context.CancelledAsync(); })
        : plugin.Observer("fixture", new Duration { Seconds = 1 }, observer);
    return Serve(plugin);
  }

  private static PluginService Serve(SdkPlugin plugin, ulong messageBytes = 1 << 20) =>
      plugin.Service(plugin.Declarations(), messageBytes, true, new Lifetime());

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
    public ValueTask Grant(ulong sequence, ulong count = 1) => Send(new InstanceSamplesRequest
    {
      Window = new WireWindow { Start = new Timestamp { Seconds = 1 }, Period = new Duration { Seconds = 1 }, Count = count, Sequence = sequence }
    });
    public ValueTask Reply(ulong id, IMessage value) => Send(new InstanceSamplesRequest { Child = new ChildResponse { Call = id, Payload = value.ToByteString() } });
    public ValueTask<InstanceSamplesResponse> Read() => output.Reader.ReadAsync();
    public async Task<SampleBatch> ReadBatch()
    {
      var response = await Read();
      Assert.Equal(InstanceSamplesResponse.FrameOneofCase.Batch, response.FrameCase);
      Assert.Empty(response.Samples);
      return response.Batch;
    }
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
