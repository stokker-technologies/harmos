// SPDX-License-Identifier: Apache-2.0

using Harmos.Context.V1;
using Visibility = Harmos.V1.Visibility;
using Xunit;

namespace Harmos.Sdk.Tests.Unit;

public sealed class UnaryPluginCalls
{
  [Fact]
  public void RefusedWatchRegistrationDoesNotReserveTheUnaryOperation()
  {
    var plugin = new Plugin("unary", "1.0.0");
    var watch = new Operation<TimeGetRequest, TimeGetResponse>("unary.read", Visibility.Public, watch: true);
    var refusal = Assert.Throws<ArgumentException>(() => plugin.Operation(watch, (_, _) => Task.FromResult(new TimeGetResponse())));
    Assert.Contains("unary.read", refusal.Message);
    var unary = new Operation<TimeGetRequest, TimeGetResponse>("unary.read", Visibility.Public);
    plugin.Operation(unary, (_, _) => Task.FromResult(new TimeGetResponse()));
    Assert.Equal("unary.read", Assert.Single(plugin.Catalog().Operations).Id);
  }
}
