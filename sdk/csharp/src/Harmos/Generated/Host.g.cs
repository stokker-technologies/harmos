// @generated from protocol descriptors; do not edit.
#nullable enable
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
namespace Harmos.Plugins;
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
  SubscriptionsSubscribe,
  TimeGet,
  TimeSleep,
}
internal static class HostContracts
{
  internal static readonly Operation<global::Harmos.Clients.InvocationsGetRequest, global::Harmos.Clients.InvocationsGetResponse> InvocationsGet = new("harmos.plugin.v1.Invocations/Get", Visibility.Public, watch: false);
  internal static readonly Operation<global::Harmos.Clients.InvocationsCancelRequest, global::Harmos.Clients.InvocationsCancelResponse> InvocationsCancel = new("harmos.plugin.v1.Invocations/Cancel", Visibility.Public, watch: false);
  internal static readonly Operation<global::Harmos.Clients.InvocationsWatchRequest, global::Harmos.Clients.InvocationsWatchResponse> InvocationsWatch = new("harmos.plugin.v1.Invocations/Watch", Visibility.Public, watch: true);
  internal static readonly Operation<global::Harmos.Clients.OperationsListRequest, global::Harmos.Clients.OperationsListResponse> OperationsList = new("harmos.plugin.v1.Operations/List", Visibility.Public, watch: false);
  internal static readonly Operation<global::Harmos.Clients.OperationsGetRequest, global::Harmos.Clients.OperationsGetResponse> OperationsGet = new("harmos.plugin.v1.Operations/Get", Visibility.Public, watch: false);
  internal static readonly Operation<global::Harmos.Clients.SettingsListRequest, global::Harmos.Clients.SettingsListResponse> PluginsSettingsList = new("harmos.plugin.v1.plugins.Settings/List", Visibility.Public, watch: false);
  internal static readonly Operation<global::Harmos.Clients.StateGetRequest, global::Harmos.Clients.StateGetResponse> PluginsStateGet = new("harmos.plugin.v1.plugins.State/Get", Visibility.Public, watch: false);
  internal static readonly Operation<global::Harmos.Clients.StatePutRequest, global::Harmos.Clients.StatePutResponse> PluginsStatePut = new("harmos.plugin.v1.plugins.State/Put", Visibility.Public, watch: false);
  internal static readonly Operation<global::Harmos.Internal.Plugin.SubscriptionsSubscribeRequest, global::Harmos.Internal.Plugin.SubscriptionsSubscribeResponse> SubscriptionsSubscribe = new("harmos.plugin.v1.Subscriptions/Subscribe", Visibility.Public, watch: true);
  internal static readonly Operation<global::Harmos.Clients.TimeGetRequest, global::Harmos.Clients.TimeGetResponse> TimeGet = new("harmos.plugin.v1.Time/Get", Visibility.Public, watch: false);
  internal static readonly Operation<global::Harmos.Clients.TimeSleepRequest, global::Harmos.Clients.TimeSleepResponse> TimeSleep = new("harmos.plugin.v1.Time/Sleep", Visibility.Public, watch: false);
}
public static class HostRequirements
{
  public static Registrar Register(this Registrar ctx, HostOperation operation) => operation switch
  {
    HostOperation.InvocationsGet => ctx.Register(HostContracts.InvocationsGet),
    HostOperation.InvocationsCancel => ctx.Register(HostContracts.InvocationsCancel),
    HostOperation.InvocationsWatch => ctx.Register(HostContracts.InvocationsWatch),
    HostOperation.OperationsList => ctx.Register(HostContracts.OperationsList),
    HostOperation.OperationsGet => ctx.Register(HostContracts.OperationsGet),
    HostOperation.PluginsSettingsList => ctx.Register(HostContracts.PluginsSettingsList),
    HostOperation.PluginsStateGet => ctx.Register(HostContracts.PluginsStateGet),
    HostOperation.PluginsStatePut => ctx.Register(HostContracts.PluginsStatePut),
    HostOperation.SubscriptionsSubscribe => ctx.Register(HostContracts.SubscriptionsSubscribe),
    HostOperation.TimeGet => ctx.Register(HostContracts.TimeGet),
    HostOperation.TimeSleep => ctx.Register(HostContracts.TimeSleep),
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
    var request = new global::Harmos.Clients.InvocationsGetRequest
    {
      Instance = instance,
    };
    var response = await context.CallAsync(HostContracts.InvocationsGet, request, policy, cancellationToken);
    return response.Invocation ?? throw new InvalidOperationException("Read InvocationsGet: Missing Payload");
  }
  public async Task<global::Harmos.InstanceState> Cancel(ulong instance, string reason, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.InvocationsCancelRequest
    {
      Instance = instance,
      Reason = reason,
    };
    var response = await context.CallAsync(HostContracts.InvocationsCancel, request, policy, cancellationToken);
    return response.Invocation ?? throw new InvalidOperationException("Read InvocationsCancel: Missing Payload");
  }
  public async IAsyncEnumerable<global::Harmos.InstanceState> Watch(ulong instance, ulong maxItemBytes, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.InvocationsWatchRequest
    {
      Instance = instance,
    };
    await foreach (var response in context.WatchAsync(HostContracts.InvocationsWatch, request, maxItemBytes, cancellationToken))
    {
      yield return response.Invocation ?? throw new InvalidOperationException("Read InvocationsWatch: Missing Payload");
    }
  }
}
public sealed class HostOperationsResource(Context context)
{
  public async Task<IReadOnlyList<global::Harmos.DiscoveredOperation>> List(OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.OperationsListRequest
    {
    };
    var response = await context.CallAsync(HostContracts.OperationsList, request, policy, cancellationToken);
    return response.Operations;
  }
  public async Task<global::Harmos.DiscoveredOperation> Get(string id, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.OperationsGetRequest
    {
      Id = id,
    };
    var response = await context.CallAsync(HostContracts.OperationsGet, request, policy, cancellationToken);
    return response.Operation ?? throw new InvalidOperationException("Read OperationsGet: Missing Payload");
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
    var request = new global::Harmos.Clients.SettingsListRequest
    {
    };
    if (name is not null)
    {
      request.Name = name;
    }
    if (at is not null)
    {
      request.At = at;
    }
    var response = await context.CallAsync(HostContracts.PluginsSettingsList, request, policy, cancellationToken);
    return response;
  }
}
public sealed class HostPluginsStateResource(Context context)
{
  public async Task<global::Harmos.Clients.StateGetResponse> Get(OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.StateGetRequest
    {
    };
    var response = await context.CallAsync(HostContracts.PluginsStateGet, request, policy, cancellationToken);
    return response;
  }
  public async Task<ulong> Put(ulong expected_revision, ByteString value, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.StatePutRequest
    {
      ExpectedRevision = expected_revision,
      Value = value,
    };
    var response = await context.CallAsync(HostContracts.PluginsStatePut, request, policy, cancellationToken);
    return response.Revision;
  }
}
public sealed class HostTimeResource(Context context)
{
  public async Task<global::Google.Protobuf.WellKnownTypes.Timestamp> Get(OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.TimeGetRequest
    {
    };
    var response = await context.CallAsync(HostContracts.TimeGet, request, policy, cancellationToken);
    return response.Now ?? throw new InvalidOperationException("Read TimeGet: Missing Payload");
  }
  public async Task<global::Harmos.Clients.TimeSleepResponse> Sleep(global::Google.Protobuf.WellKnownTypes.Duration duration, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.TimeSleepRequest
    {
      Duration = duration,
    };
    var response = await context.CallAsync(HostContracts.TimeSleep, request, policy, cancellationToken);
    return response;
  }
}
