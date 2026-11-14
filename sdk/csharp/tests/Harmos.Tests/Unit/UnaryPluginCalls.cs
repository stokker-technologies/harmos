// SPDX-License-Identifier: Apache-2.0

using System.Linq;
using System.Threading.Tasks;
using System;
using Harmos.Plugins;
using Harmos;
using Xunit;
using SdkPlugin = Harmos.Plugins.Plugin;
using Visibility = Harmos.Visibility;

namespace Harmos.Tests.Unit;

public sealed class UnaryPluginCalls
{
  [Fact]
  public void RefusedWatchRegistrationDoesNotReserveTheUnaryOperation()
  {
    var plugin = new SdkPlugin("unary", "1.0.0");
    var watch = new Operation<TimeGetRequest, TimeGetResponse>("unary.read", Visibility.Public, watch: true);
    var refusal = Assert.Throws<ArgumentException>(() => plugin.Operation(watch, (_, _) => Task.FromResult(new TimeGetResponse())));
    Assert.Contains("unary.read", refusal.Message);
    var unary = new Operation<TimeGetRequest, TimeGetResponse>("unary.read", Visibility.Public);
    plugin.Operation(unary, (_, _) => Task.FromResult(new TimeGetResponse()));
    Assert.Equal("unary.read", Assert.Single(plugin.Declarations().Operations).Id);
  }
}
