// SPDX-License-Identifier: Apache-2.0

using System.IO;
using System.Linq;
using System.Net;
using System.Threading.Tasks;
using System.Threading;
using System;
using Harmos.Clients;
using Harmos.Internal.Plugin;
using Harmos.Plugins;
using Harmos;
using Grpc.Core;
using Grpc.Net.Client;
using Xunit;

namespace Harmos.Tests.Unit;

[Collection("Console and Environment")]
public sealed class UnaryPluginCalls
{
  [Fact]
  public async Task RunAnnouncesOnlyManagedLaunchesUntilCleanStop()
  {
    foreach (var managed in new[] { false, true })
    {
      var stdout = Console.Out;
      var stderr = Console.Error;
      var configured = Environment.GetEnvironmentVariable("HARMOS_ADDRESS");
      using var output = new StringWriter();
      using var diagnostics = new StringWriter();
      using var cancellation = new CancellationTokenSource();
      Task? serving = null;
      Console.SetOut(output);
      Console.SetError(diagnostics);
      try
      {
        var plugin = new Harmos.Plugins.Plugin("example", "1.0.0");
        string address;
        await using (var listener = await plugin.StartAsync("127.0.0.1:0"))
          address = listener.Address;
        Environment.SetEnvironmentVariable("HARMOS_ADDRESS", managed ? address : null);
        serving = plugin.RunAsync(address, cancellationToken: cancellation.Token);
        using var channel = GrpcChannel.ForAddress($"http://{address}");
        var client = new Instance.InstanceClient(channel);
        await client.GetAsync(new InstanceGetRequest { Revision = 1 }, new CallOptions().WithWaitForReady());
        await client.StopAsync(new InstanceStopRequest());
        await serving;
        Assert.Equal(managed ? $"harmos listening {address}{Environment.NewLine}" : "", output.ToString());
        Assert.Equal("", diagnostics.ToString());
      }
      finally
      {
        Environment.SetEnvironmentVariable("HARMOS_ADDRESS", configured);
        Console.SetOut(stdout);
        Console.SetError(stderr);
        cancellation.Cancel();
        if (serving is not null)
        {
          try
          {
            await serving;
          }
          catch (OperationCanceledException) when (cancellation.IsCancellationRequested)
          {
          }
        }
      }
    }
  }

  [Fact]
  public async Task StartReturnsTheBoundAddressWithoutWritingOutput()
  {
    var stdout = Console.Out;
    var stderr = Console.Error;
    using var output = new StringWriter();
    using var diagnostics = new StringWriter();
    Console.SetOut(output);
    Console.SetError(diagnostics);
    try
    {
      await using (var listener = await new Harmos.Plugins.Plugin("example", "1.0.0").StartAsync("127.0.0.1:0"))
      {
        var endpoint = IPEndPoint.Parse(listener.Address);
        Assert.True(IPAddress.IsLoopback(endpoint.Address));
        Assert.True(endpoint.Port > 0);
      }
      Assert.Equal("", output.ToString());
      Assert.Equal("", diagnostics.ToString());
    }
    finally
    {
      Console.SetOut(stdout);
      Console.SetError(stderr);
    }
  }

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
    Assert.Equal(new[] { "0A0A696E737472756D656E741001", "0A08706172616C6C656C1802", "0A046F70656E2001" },
        declarations.Lanes.Select(lane => Convert.ToHexString(Google.Protobuf.MessageExtensions.ToByteArray(lane))));
    Assert.Equal("instrument", Assert.Single(declarations.Operations).Lane);
    Assert.Throws<ArgumentException>(() => Lane.Serial(""));
    Assert.Throws<ArgumentException>(() => Lane.Concurrent("instrument", 0));
    Assert.Throws<ArgumentException>(() => plugin.Lane(instrument));
    var undeclared = new Harmos.Plugins.Plugin("example", "1.0.0").Register(ctx => ctx.Register(operation, (_, _) => Task.FromResult(new TimeGetResponse())));
    Assert.Throws<ArgumentException>(() => undeclared.Declarations());
  }
}

[CollectionDefinition("Console and Environment", DisableParallelization = true)]
public sealed class ConsoleAndEnvironment
{
}
