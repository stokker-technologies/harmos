// SPDX-License-Identifier: Apache-2.0

using System.Collections.Generic;
using System.Threading;
using System.Threading.Channels;
using System.Threading.Tasks;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Harmos.Clients;
using Harmos.Internal.Plugin;
using Harmos.Plugins;
using Xunit;

namespace Harmos.Tests.Unit;

public sealed class Subscriptions
{
  [Fact]
  public async Task TypedRowsAndGapsKeepDemandClosureIndependent()
  {
    var output = Channel.CreateUnbounded<InstanceInvokeResponse>();
    using var cancellation = new CancellationTokenSource();
    var contract = HostContracts.SubscriptionsSubscribe.Contract;
    var context = new Context(new Request { Operation = "fixture.read", Scope = "unit/first" }, "fixture",
        new Dictionary<string, ByteString> { [contract.Id] = contract.Fingerprint }, cancellation, output.Writer);
    var first = await Open(context, output.Reader);
    var second = await Open(context, output.Reader);
    var source = new Timestamp { Seconds = 7, Nanos = 123456789 };
    var received = new Timestamp { Seconds = 11, Nanos = 987654321 };
    var row = new SubscriptionRow { Payload = new TimeGetResponse { Now = source }.ToByteString(), Source = source, Received = received };

    var reading = first.Subscription.NextAsync();
    Assert.Equal(first.Call, (await output.Reader.ReadAsync()).ChildDemand.Call);
    await context.Reply(new ChildResponse { Call = first.Call, Item = new SubscriptionsSubscribeResponse { Row = row }.ToByteString() });
    var item = Assert.IsType<StreamRow<TimeGetResponse>>(await reading);
    Assert.Equal((source, source, received), (item.Value.Now, item.Source, item.Received));
    reading = first.Subscription.NextAsync();
    await output.Reader.ReadAsync();
    await context.Reply(new ChildResponse { Call = first.Call, Item = new SubscriptionsSubscribeResponse { Missed = 17 }.ToByteString() });
    Assert.Equal(17UL, Assert.IsType<Gap<TimeGetResponse>>(await reading).Missed);
    await first.Subscription.CloseAsync();
    Assert.Equal(first.Call, (await output.Reader.ReadAsync()).ChildCancel.Call);
    await first.Subscription.CloseAsync();
    Assert.Null(await first.Subscription.NextAsync());

    reading = second.Subscription.NextAsync();
    Assert.Equal(second.Call, (await output.Reader.ReadAsync()).ChildDemand.Call);
    await context.Reply(new ChildResponse { Call = second.Call, Item = new SubscriptionsSubscribeResponse { Row = row }.ToByteString() });
    Assert.Equal(source, Assert.IsType<StreamRow<TimeGetResponse>>(await reading).Source);
    reading = second.Subscription.NextAsync();
    await output.Reader.ReadAsync();
    await context.Cancel("Fixture Scope Ended");
    await Assert.ThrowsAsync<PluginFailureException>(() => reading);
    await second.Subscription.CloseAsync();
  }

  private static async Task<(Subscription<TimeGetResponse> Subscription, ulong Call)> Open(Context context, ChannelReader<InstanceInvokeResponse> output)
  {
    var subscribing = context.SubscribeAsync<TimeGetResponse>("fixture.samples", 4096);
    var child = (await output.ReadAsync()).Child;
    Assert.True(child.Watch);
    Assert.Equal(child.Call, (await output.ReadAsync()).ChildDemand.Call);
    await context.Reply(new ChildResponse { Call = child.Call, Item = new SubscriptionsSubscribeResponse { Opened = true }.ToByteString() });
    return (await subscribing, child.Call);
  }
}
