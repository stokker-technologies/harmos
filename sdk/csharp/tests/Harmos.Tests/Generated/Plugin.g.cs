// @generated from protocol descriptors; do not edit.
#nullable enable
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Harmos;
using Harmos.Clients;
using Harmos.Plugins;
using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
namespace Fixture.Plugins;
public enum HostOperation
{
  InvocationsGet,
  InvocationsCancel,
  InvocationsWatch,
  OperationsList,
  OperationsGet,
  PluginsSettingsList,
  TimeGet,
  TimeSleep,
}
internal static class HostContracts
{
}
public static class HostRequirements
{
  public static Registrar Register(this Registrar ctx, HostOperation operation) => operation switch
  {
    HostOperation.InvocationsGet => ctx.Register(Harmos.Plugins.HostOperation.InvocationsGet),
    HostOperation.InvocationsCancel => ctx.Register(Harmos.Plugins.HostOperation.InvocationsCancel),
    HostOperation.InvocationsWatch => ctx.Register(Harmos.Plugins.HostOperation.InvocationsWatch),
    HostOperation.OperationsList => ctx.Register(Harmos.Plugins.HostOperation.OperationsList),
    HostOperation.OperationsGet => ctx.Register(Harmos.Plugins.HostOperation.OperationsGet),
    HostOperation.PluginsSettingsList => ctx.Register(Harmos.Plugins.HostOperation.PluginsSettingsList),
    HostOperation.TimeGet => ctx.Register(Harmos.Plugins.HostOperation.TimeGet),
    HostOperation.TimeSleep => ctx.Register(Harmos.Plugins.HostOperation.TimeSleep),
    _ => throw new ArgumentOutOfRangeException(nameof(operation), "Declare Host Requirement: Unknown Operation")
  };
}
public sealed class HostRoot(Context context)
{
  public HostInvocationsResource Invocations { get; } = new(context);
  public HostOperationsResource Operations { get; } = new(context);
  public HostPluginsResource Plugins { get; } = new(context);
  public HostTimeResource Time { get; } = new(context);
}
public sealed class HostInvocationsResource(Context context)
{
  public async Task<InstanceState> Get(ulong instance, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Invocations.Get(instance, policy, cancellationToken);
  }
  public async Task<InstanceState> Cancel(ulong instance, string reason, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Invocations.Cancel(instance, reason, policy, cancellationToken);
  }
  public async IAsyncEnumerable<InstanceState> Watch(ulong instance, ulong maxItemBytes, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    await foreach (var response in context.Host.Invocations.Watch(instance, maxItemBytes, cancellationToken))
    {
      yield return response;
    }
  }
}
public sealed class HostOperationsResource(Context context)
{
  public async Task<IReadOnlyList<DiscoveredOperation>> List(OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Operations.List(policy, cancellationToken);
  }
  public async Task<DiscoveredOperation> Get(string id, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Operations.Get(id, policy, cancellationToken);
  }
}
public sealed class HostPluginsResource(Context context)
{
  public HostPluginsSettingsResource Settings { get; } = new(context);
}
public sealed class HostPluginsSettingsResource(Context context)
{
  public async Task<SettingsListResponse> List(string? name = null, ReadCut? at = null, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Plugins.Settings.List(name, at, policy, cancellationToken);
  }
}
public sealed class HostTimeResource(Context context)
{
  public async Task<Timestamp> Get(OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Time.Get(policy, cancellationToken);
  }
  public async Task<TimeSleepResponse> Sleep(Duration duration, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Time.Sleep(duration, policy, cancellationToken);
  }
}
public sealed class PluginContext(Harmos.Plugins.Context context)
{
  public HostRoot Host { get; } = new(context);
}
