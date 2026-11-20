// SPDX-License-Identifier: Apache-2.0

using System.Diagnostics;
using System.Net;
using System.Threading.Tasks;
using System;
using Grpc.Net.Client;
using Harmos.Internal.Plugin;
using Harmos.Tests.Support;
using Xunit;

namespace Harmos.Tests.Integration;

public sealed class ManagedLaunch
{
  [Fact]
  public async Task ManagedRunAnnouncesOnceAndWritesNothing()
  {
    await using var bootstrap = await Bootstrap.StartAsync();
    using var process = Process.Start(bootstrap.Launch(ReferenceHost.Artifact("HARMOS_CSHARP_PLUGIN")))
        ?? throw new InvalidOperationException("Plugin Did Not Start");
    var stdout = process.StandardOutput.ReadToEndAsync();
    var stderr = process.StandardError.ReadToEndAsync();
    try
    {
      var announce = await bootstrap.Announces.ReadAsync();
      Assert.Equal(Bootstrap.Token, announce.Token);
      var address = IPEndPoint.Parse(announce.Address);
      Assert.True(IPAddress.IsLoopback(address.Address));
      Assert.True(address.Port > 0);
      using var channel = GrpcChannel.ForAddress($"http://{announce.Address}");
      await new Instance.InstanceClient(channel).StopAsync(new InstanceStopRequest { Reason = "Story Stop" });
      await process.WaitForExitAsync();
    }
    finally
    {
      if (!process.HasExited)
        process.Kill(entireProcessTree: true);
    }
    Assert.Equal(0, process.ExitCode);
    Assert.Equal("", await stdout);
    Assert.Equal("", await stderr);
    Assert.False(bootstrap.Announces.TryRead(out _));
  }

  [Fact]
  public async Task RefusedAnnounceFailsTheStartWithTheHostMessage()
  {
    await using var bootstrap = await Bootstrap.StartAsync(refusal: "Launch Credential Is Invalid");
    var (exitCode, stderr) = await ExitAsync(bootstrap.Launch(ReferenceHost.Artifact("HARMOS_CSHARP_PLUGIN")));
    Assert.NotEqual(0, exitCode);
    Assert.Contains("Launch Credential Is Invalid", stderr);
  }

  [Fact]
  public async Task OneLaunchVariableIsAStartErrorNamingTheOther()
  {
    foreach (var (present, missing) in new[] { ("HARMOS_HOST_ENDPOINT", "HARMOS_LAUNCH_TOKEN"), ("HARMOS_LAUNCH_TOKEN", "HARMOS_HOST_ENDPOINT") })
    {
      var start = new ProcessStartInfo(ReferenceHost.Artifact("HARMOS_CSHARP_PLUGIN")) { RedirectStandardOutput = true, RedirectStandardError = true };
      start.Environment[present] = "127.0.0.1:9";
      start.Environment.Remove(missing);
      var (exitCode, stderr) = await ExitAsync(start);
      Assert.NotEqual(0, exitCode);
      Assert.Contains($"Set {missing}", stderr);
    }
  }

  private static async Task<(int ExitCode, string Stderr)> ExitAsync(ProcessStartInfo start)
  {
    using var process = Process.Start(start) ?? throw new InvalidOperationException("Plugin Did Not Start");
    var stdout = process.StandardOutput.ReadToEndAsync();
    var stderr = process.StandardError.ReadToEndAsync();
    await process.WaitForExitAsync();
    await stdout;
    return (process.ExitCode, await stderr);
  }
}
