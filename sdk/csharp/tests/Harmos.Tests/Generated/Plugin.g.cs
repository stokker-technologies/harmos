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
  PluginsStateGet,
  PluginsStatePut,
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
    HostOperation.PluginsStateGet => ctx.Register(Harmos.Plugins.HostOperation.PluginsStateGet),
    HostOperation.PluginsStatePut => ctx.Register(Harmos.Plugins.HostOperation.PluginsStatePut),
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
  public async Task<global::Harmos.InstanceState> Get(ulong instance, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Invocations.Get(instance, policy, cancellationToken);
  }
  public async Task<global::Harmos.InstanceState> Cancel(ulong instance, string reason, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Invocations.Cancel(instance, reason, policy, cancellationToken);
  }
  public async IAsyncEnumerable<global::Harmos.InstanceState> Watch(ulong instance, ulong maxItemBytes, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    await foreach (var response in context.Host.Invocations.Watch(instance, maxItemBytes, cancellationToken))
    {
      yield return response;
    }
  }
}
public sealed class HostOperationsResource(Context context)
{
  public async Task<IReadOnlyList<global::Harmos.DiscoveredOperation>> List(OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Operations.List(policy, cancellationToken);
  }
  public async Task<global::Harmos.DiscoveredOperation> Get(string id, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Operations.Get(id, policy, cancellationToken);
  }
}
public sealed class HostPluginsResource(Context context)
{
  public HostPluginsSettingsResource Settings { get; } = new(context);
  public HostPluginsStateResource State { get; } = new(context);
}
public sealed class HostPluginsSettingsResource(Context context)
{
  public async Task<global::Harmos.Clients.SettingsListResponse> List(string? name = null, global::Harmos.ReadCut? at = null, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Plugins.Settings.List(name, at, policy, cancellationToken);
  }
}
public sealed class HostPluginsStateResource(Context context)
{
  public async Task<global::Harmos.Clients.StateGetResponse> Get(OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Plugins.State.Get(policy, cancellationToken);
  }
  public async Task<ulong> Put(ulong expected_revision, ByteString value, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Plugins.State.Put(expected_revision, value, policy, cancellationToken);
  }
}
public sealed class HostTimeResource(Context context)
{
  public async Task<global::Google.Protobuf.WellKnownTypes.Timestamp> Get(OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Time.Get(policy, cancellationToken);
  }
  public async Task<global::Harmos.Clients.TimeSleepResponse> Sleep(global::Google.Protobuf.WellKnownTypes.Duration duration, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    return await context.Host.Time.Sleep(duration, policy, cancellationToken);
  }
}
public sealed class PluginContext(Harmos.Plugins.Context context)
{
  public HostRoot Host { get; } = new(context);
}
