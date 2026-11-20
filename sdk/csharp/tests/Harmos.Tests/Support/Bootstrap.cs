// SPDX-License-Identifier: Apache-2.0

using System.Diagnostics;
using System.Linq;
using System.Net;
using System.Threading.Channels;
using System.Threading.Tasks;
using System;
using Grpc.Core;
using Harmos.Internal.Plugin;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Server.Kestrel.Core;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;

namespace Harmos.Tests.Support;

/// <summary>A minimal host bootstrap listener: serves Launches/Announce on loopback and records each call.</summary>
internal sealed class Bootstrap : IAsyncDisposable
{
  internal const string Token = "story-launch-token";

  private readonly WebApplication app;

  private Bootstrap(WebApplication app, Channel<LaunchesAnnounceRequest> announces)
  {
    this.app = app;
    Announces = announces.Reader;
    Endpoint = new Uri(app.Urls.Single()).Authority;
  }

  internal ChannelReader<LaunchesAnnounceRequest> Announces
  {
    get;
  }

  internal string Endpoint
  {
    get;
  }

  /// <summary>Serve Announce; a refusal answers every call with UNAUTHENTICATED.</summary>
  internal static async Task<Bootstrap> StartAsync(string? refusal = null)
  {
    var announces = Channel.CreateUnbounded<LaunchesAnnounceRequest>();
    var builder = WebApplication.CreateSlimBuilder();
    builder.Logging.ClearProviders();
    builder.WebHost.ConfigureKestrel(options => options.Listen(IPAddress.Loopback, 0, listener => listener.Protocols = HttpProtocols.Http2));
    builder.Services.AddGrpc();
    builder.Services.AddSingleton(new Service(announces.Writer, refusal));
    var app = builder.Build();
    app.MapGrpcService<Service>();
    await app.StartAsync();
    return new Bootstrap(app, announces);
  }

  /// <summary>A plugin start carrying both launch variables, with redirected output.</summary>
  internal ProcessStartInfo Launch(string plugin)
  {
    var start = new ProcessStartInfo(plugin) { RedirectStandardOutput = true, RedirectStandardError = true };
    start.Environment["HARMOS_HOST_ENDPOINT"] = Endpoint;
    start.Environment["HARMOS_LAUNCH_TOKEN"] = Token;
    return start;
  }

  public async ValueTask DisposeAsync()
  {
    try
    {
      await app.StopAsync();
    }
    finally
    {
      await app.DisposeAsync();
    }
  }

  private sealed class Service(ChannelWriter<LaunchesAnnounceRequest> announces, string? refusal) : Launches.LaunchesBase
  {
    public override Task<LaunchesAnnounceResponse> Announce(LaunchesAnnounceRequest request, ServerCallContext context)
    {
      announces.TryWrite(request);
      if (refusal is not null)
        throw new RpcException(new Status(StatusCode.Unauthenticated, refusal));
      return Task.FromResult(new LaunchesAnnounceResponse());
    }
  }
}
