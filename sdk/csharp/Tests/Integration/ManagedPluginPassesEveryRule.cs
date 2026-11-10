// SPDX-License-Identifier: Apache-2.0

using Harmos.Sdk.Tests.Support;
using Xunit;
using Xunit.Abstractions;

namespace Harmos.Sdk.Tests.Integration;

public sealed class ManagedPluginPassesEveryRule(ITestOutputHelper output)
{
  [Fact]
  public async Task PluginPassesEveryRuleWithManagedAndRemoteLifecycles()
  {
    // Locate the Built Reference Host and SDK-Authored Plugin.
    var host = ReferenceHost.Artifact("HARMOS_CONFORMANCE_HOST");
    var plugin = ReferenceHost.Artifact("HARMOS_CSHARP_PLUGIN");

    // Run the Same Managed Lifecycle and Protocol Cases as Every Language SDK.
    var result = await ReferenceHost.RunAsync(host, plugin);
    output.WriteLine(result.Evidence);

    // Require All Cases and Successful Managed Shutdown.
    Assert.True(result.ExitCode == 0, result.Evidence);

    // Run the Same Plugin Remotely and Verify Stop Leaves It Available.
    var remote = await ReferenceHost.RunRemoteAsync(host, plugin);
    output.WriteLine(remote.Evidence);
    Assert.True(remote.ExitCode == 0, remote.Evidence);
    Assert.True(remote.StillServing);
  }
}
