// SPDX-License-Identifier: Apache-2.0

using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Threading.Tasks;
using System.Threading;
using System;
using IOFile = System.IO.File;

namespace Harmos.Tests.Support;

internal static class ReferenceHost
{
  internal static string Artifact(string name)
  {
    var path = Environment.GetEnvironmentVariable(name);
    if (string.IsNullOrEmpty(path))
    {
      var directory = new DirectoryInfo(AppContext.BaseDirectory);
      while (directory is not null && !IOFile.Exists(Path.Combine(directory.FullName, "Harmos.slnx")))
        directory = directory.Parent;
      var sdk = directory?.FullName ?? throw new InvalidOperationException("Missing C# Solution");
      var configuration = typeof(ReferenceHost).Assembly.GetCustomAttribute<AssemblyConfigurationAttribute>()!.Configuration;
      path = name == "HARMOS_CSHARP_PLUGIN"
        ? Path.GetFullPath(IOFile.ReadAllText(Path.Combine(sdk, "artifacts", "paths", configuration, "Harmos.Conformance.txt")).Trim(), sdk)
        : Path.GetFullPath(Path.Combine(sdk, "..", "..", "target", configuration == "Release" ? "release" : "debug", OperatingSystem.IsWindows() ? "harmos-conformance.exe" : "harmos-conformance"));
    }
    if (string.IsNullOrEmpty(path) || !IOFile.Exists(path))
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
