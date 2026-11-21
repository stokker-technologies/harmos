// SPDX-License-Identifier: Apache-2.0

using System.Net;
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
  public async Task StartReturnsTheBoundLoopbackAddress()
  {
    await using var listener = await new Harmos.Plugins.Plugin("example", "1.0.0").StartAsync("127.0.0.1:0");
    var endpoint = IPEndPoint.Parse(listener.Address);
    Assert.True(IPAddress.IsLoopback(endpoint.Address));
    Assert.True(endpoint.Port > 0);
  }

  [Fact]
  public void RefusedWatchRegistrationDoesNotReserveTheUnaryOperation()
  {
    var plugin = new Harmos.Plugins.Plugin("unary", "1.0.0");
    var watch = new Operation<TimeGetRequest, TimeGetResponse>("unary.read", Visibility.Public, watch: true);
    Assert.Throws<ArgumentException>(() => plugin.Register(ctx => ctx.Register(watch, (_, _) => Task.FromResult(new TimeGetResponse()))));
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

  [Fact]
  public void LanesEncodeFactsAndRefuseInvalidDeclarations()
  {
    var instrument = Lane.Serial("instrument");
    var parallel = Lane.Concurrent("parallel", 2);
    var open = Lane.Unbounded("open");
    var operation = new Operation<TimeGetRequest, TimeGetResponse>("example.measure", Visibility.Public, lane: instrument);
    var plugin = new Harmos.Plugins.Plugin("example", "1.0.0").Lane(instrument).Lane(parallel).Lane(open)
        .Register(ctx => ctx.Register(operation, (_, _) => Task.FromResult(new TimeGetResponse())));
    var declarations = plugin.Declarations();
    Assert.Equal("instrument", Assert.Single(declarations.Operations).Lane);
    Assert.Throws<ArgumentException>(() => Lane.Serial(""));
    Assert.Throws<ArgumentException>(() => Lane.Concurrent("instrument", 0));
    Assert.Throws<ArgumentException>(() => plugin.Lane(instrument));
    var undeclared = new Harmos.Plugins.Plugin("example", "1.0.0").Register(ctx => ctx.Register(operation, (_, _) => Task.FromResult(new TimeGetResponse())));
    Assert.Throws<ArgumentException>(() => undeclared.Declarations());
  }
}
