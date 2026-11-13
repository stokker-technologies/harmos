// SPDX-License-Identifier: Apache-2.0

using System.Reflection;
using Xunit;

namespace Harmos.Tests;

public sealed class ProtocolAudience
{
  [Fact]
  public void PluginAndClientAssembliesExposeOnlyTheirOwnServiceStubs()
  {
    var plugin = typeof(Sdk.Plugin).Assembly;
    Assert.NotNull(plugin.GetType("Harmos.Plugin.V1.Instance+InstanceBase"));
    Assert.Null(plugin.GetType("Harmos.Plugin.V1.Instance+InstanceClient"));
    Assert.Null(plugin.GetType("Harmos.Context.V1.Time+TimeClient"));
    Assert.Null(plugin.GetType("Harmos.V1.Invocations+InvocationsClient"));
    Assert.Null(plugin.GetType("Harmos.Sdk.Client"));

    var configuration = new DirectoryInfo(AppContext.BaseDirectory).Parent!.Name;
    var directory = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../Harmos/bin"));
    var client = Assembly.LoadFrom(Path.Combine(directory, "Client", configuration, "net10.0", "Harmos.Client.dll"));
    Assert.NotNull(client.GetType("Harmos.Sdk.Client"));
    Assert.NotNull(client.GetType("Harmos.V1.Invocations+InvocationsClient"));
    Assert.Null(client.GetType("Harmos.V1.Invocations+InvocationsBase"));
    Assert.Null(client.GetType("Harmos.Sdk.Plugin"));
    Assert.Null(client.GetType("Harmos.Plugin.V1.Instance"));
    Assert.Null(client.GetType("Harmos.Context.V1.TimeGetRequest"));

    var host = Assembly.LoadFrom(Path.Combine(directory, "Host", configuration, "net10.0", "Harmos.Host.dll"));
    Assert.NotNull(host.GetType("Harmos.Sdk.Plugin"));
    Assert.NotNull(host.GetType("Harmos.Sdk.Client"));
    Assert.NotNull(host.GetType("Harmos.Plugin.V1.Instance+InstanceClient"));
    Assert.NotNull(host.GetType("Harmos.Plugin.V1.Instance+InstanceBase"));
    Assert.NotNull(host.GetType("Harmos.V1.Invocations+InvocationsClient"));
    Assert.NotNull(host.GetType("Harmos.V1.Invocations+InvocationsBase"));
    Assert.NotNull(host.GetType("Harmos.Context.V1.TimeGetRequest"));
    Assert.Null(host.GetType("Harmos.Context.V1.Time+TimeClient"));
  }
}
