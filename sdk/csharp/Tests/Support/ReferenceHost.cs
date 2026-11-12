using Harmos.Context.V1;
// SPDX-License-Identifier: Apache-2.0

using System.Diagnostics;

namespace Harmos.Sdk.Tests.Support;

internal static class ReferenceHost
{
  internal static string Artifact(string name)
  {
    var path = Environment.GetEnvironmentVariable(name);
    if (string.IsNullOrEmpty(path) || !File.Exists(path))
      throw new InvalidOperationException($"Missing Build Output: {name}; Run mise run build -- --language csharp");
    return path;
  }

  internal static async Task<(int ExitCode, string Evidence)> RunAsync(string host, string plugin)
  {
    var start = new ProcessStartInfo(host) { RedirectStandardOutput = true, RedirectStandardError = true };
    start.ArgumentList.Add("--plugin");
    start.ArgumentList.Add(plugin);
    // The Reference Host Owns Startup, Per-Case and Shutdown Transport Hang Guards.
    using var process = Process.Start(start) ?? throw new InvalidOperationException("Reference Host Did Not Start");
    var stdout = process.StandardOutput.ReadToEndAsync();
    var stderr = process.StandardError.ReadToEndAsync();
    await process.WaitForExitAsync();
    return (process.ExitCode, await stdout + await stderr);
  }

  internal static async Task<(int ExitCode, string Evidence, bool StillServing)> RunRemoteAsync(string host, string plugin)
  {
    var start = new ProcessStartInfo(plugin) { RedirectStandardOutput = true, RedirectStandardError = true };
    start.ArgumentList.Add("--remote");
    start.Environment["HARMOS_ADDRESS"] = "127.0.0.1:0";
    using var server = Process.Start(start) ?? throw new InvalidOperationException("Remote Plugin Did Not Start");
    var diagnostics = server.StandardError.ReadToEndAsync();
    (int ExitCode, string Evidence, bool StillServing) result;
    try
    {
      // Guard Only Transport Setup; Scenario TimeGetResponse Still Comes Exclusively from Host Ticks.
      using var startup = new CancellationTokenSource(TimeSpan.FromSeconds(30));
      var readiness = await server.StandardOutput.ReadLineAsync(startup.Token) ?? throw new InvalidOperationException("Missing Remote Readiness");
      const string prefix = "harmos listening ";
      if (!readiness.StartsWith(prefix, StringComparison.Ordinal))
        throw new InvalidOperationException($"Invalid Readiness: {readiness}");
      _ = server.StandardOutput.ReadToEndAsync();
      var request = new ProcessStartInfo(host) { RedirectStandardOutput = true, RedirectStandardError = true };
      request.ArgumentList.Add("--address");
      request.ArgumentList.Add(readiness[prefix.Length..]);
      using var process = Process.Start(request) ?? throw new InvalidOperationException("Reference Host Did Not Start");
      var stdout = process.StandardOutput.ReadToEndAsync();
      var stderr = process.StandardError.ReadToEndAsync();
      await process.WaitForExitAsync();
      result = (process.ExitCode, await stdout + await stderr, !server.HasExited);
    }
    finally
    {
      if (!server.HasExited)
        server.Kill(entireProcessTree: true);
      await server.WaitForExitAsync();
      await diagnostics;
    }
    return (result.ExitCode, result.Evidence + await diagnostics, result.StillServing);
  }
}
