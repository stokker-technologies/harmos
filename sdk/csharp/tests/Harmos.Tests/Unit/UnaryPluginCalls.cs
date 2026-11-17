// SPDX-License-Identifier: Apache-2.0

using System.Linq;
using System.Threading.Tasks;
using System;
using Harmos.Clients;
using Harmos.Plugins;
using Harmos;
using Xunit;

namespace Harmos.Tests.Unit;

public sealed class UnaryPluginCalls
{
  [Fact]
  public void RefusedWatchRegistrationDoesNotReserveTheUnaryOperation()
  {
    var plugin = new Harmos.Plugins.Plugin("unary", "1.0.0");
    var watch = new Operation<TimeGetRequest, TimeGetResponse>("unary.read", Visibility.Public, watch: true);
    var refusal = Assert.Throws<ArgumentException>(() => plugin.Register(ctx => ctx.Register(watch, (_, _) => Task.FromResult(new TimeGetResponse()))));
    Assert.Contains("unary.read", refusal.Message);
    var unary = new Operation<TimeGetRequest, TimeGetResponse>("unary.read", Visibility.Public);
    plugin.Register(ctx => ctx.Register(unary, (_, _) => Task.FromResult(new TimeGetResponse())));
    Assert.Equal("unary.read", Assert.Single(plugin.Declarations().Operations).Id);
  }

  [Fact]
  public void CatalogsAccumulateBeforeHandlersRun()
  {
    var entered = false;
    Registration catalog = ctx => ctx.Register(
        new Operation<TimeGetRequest, TimeGetResponse>("catalog.read", Visibility.Public),
        (_, _) =>
        {
          entered = true;
          return Task.FromResult(new TimeGetResponse());
        });
    var plugin = new Harmos.Plugins.Plugin("catalog", "1.0.0").Register(ctx => ctx.Register(catalog));
    Assert.Equal("catalog.read", Assert.Single(plugin.Declarations().Operations).Id);
    Assert.False(entered);
  }

}
