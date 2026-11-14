// @generated from protocol descriptors; do not edit.
#nullable enable
using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using System.Runtime.CompilerServices;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
namespace Harmos.Plugins;
public enum HostOperation
{
  InvocationsGet,
  InvocationsCancel,
  InvocationsWatch,
  OperationsList,
  OperationsGet,
  SettingsList,
  TimeGet,
  TimeSleep,
}
internal static class HostContracts
{
  internal static readonly Operation<InvocationsGetRequest, InvocationsGetResponse> InvocationsGet = new("harmos.plugin.host.v1.Invocations/Get", Visibility.Public, watch: false);
  internal static readonly Operation<InvocationsCancelRequest, InvocationsCancelResponse> InvocationsCancel = new("harmos.plugin.host.v1.Invocations/Cancel", Visibility.Public, watch: false);
  internal static readonly Operation<InvocationsWatchRequest, InvocationsWatchResponse> InvocationsWatch = new("harmos.plugin.host.v1.Invocations/Watch", Visibility.Public, watch: true);
  internal static readonly Operation<OperationsListRequest, OperationsListResponse> OperationsList = new("harmos.plugin.host.v1.Operations/List", Visibility.Public, watch: false);
  internal static readonly Operation<OperationsGetRequest, OperationsGetResponse> OperationsGet = new("harmos.plugin.host.v1.Operations/Get", Visibility.Public, watch: false);
  internal static readonly Operation<SettingsListRequest, SettingsListResponse> SettingsList = new("harmos.plugin.host.v1.Settings/List", Visibility.Public, watch: false);
  internal static readonly Operation<TimeGetRequest, TimeGetResponse> TimeGet = new("harmos.plugin.host.v1.Time/Get", Visibility.Public, watch: false);
  internal static readonly Operation<TimeSleepRequest, TimeSleepResponse> TimeSleep = new("harmos.plugin.host.v1.Time/Sleep", Visibility.Public, watch: false);
}
public static class HostRequirements
{
  public static Plugin Requires(this Plugin plugin, HostOperation operation) => operation switch
  {
    HostOperation.InvocationsGet => plugin.Requires(HostContracts.InvocationsGet),
    HostOperation.InvocationsCancel => plugin.Requires(HostContracts.InvocationsCancel),
    HostOperation.InvocationsWatch => plugin.Requires(HostContracts.InvocationsWatch),
    HostOperation.OperationsList => plugin.Requires(HostContracts.OperationsList),
    HostOperation.OperationsGet => plugin.Requires(HostContracts.OperationsGet),
    HostOperation.SettingsList => plugin.Requires(HostContracts.SettingsList),
    HostOperation.TimeGet => plugin.Requires(HostContracts.TimeGet),
    HostOperation.TimeSleep => plugin.Requires(HostContracts.TimeSleep),
    _ => throw new ArgumentOutOfRangeException(nameof(operation), "Declare Host Requirement: Unknown Operation")
  };
}
public sealed class HostRoot(Context context)
{
  public HostInvocationsResource Invocations { get; } = new(context);
  public HostOperationsResource Operations { get; } = new(context);
  public HostSettingsResource Settings { get; } = new(context);
  public HostTimeResource Time { get; } = new(context);
}
public sealed class HostInvocationsResource(Context context)
{
  public async Task<InstanceState> Get(ulong instance, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new InvocationsGetRequest
    {
      Instance = instance,
    };
    var response = await context.CallAsync(HostContracts.InvocationsGet, request, policy, cancellationToken);
    return response.Invocation ?? throw new InvalidOperationException("Read InvocationsGet: Missing Payload");
  }
  public async Task<InstanceState> Cancel(ulong instance, string reason, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new InvocationsCancelRequest
    {
      Instance = instance,
      Reason = reason,
    };
    var response = await context.CallAsync(HostContracts.InvocationsCancel, request, policy, cancellationToken);
    return response.Invocation ?? throw new InvalidOperationException("Read InvocationsCancel: Missing Payload");
  }
  public async IAsyncEnumerable<InstanceState> Watch(ulong instance, ulong maxItemBytes, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new InvocationsWatchRequest
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
  public async Task<IReadOnlyList<DiscoveredOperation>> List(OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new OperationsListRequest
    {
    };
    var response = await context.CallAsync(HostContracts.OperationsList, request, policy, cancellationToken);
    return response.Operations;
  }
  public async Task<DiscoveredOperation> Get(string id, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new OperationsGetRequest
    {
      Id = id,
    };
    var response = await context.CallAsync(HostContracts.OperationsGet, request, policy, cancellationToken);
    return response.Operation ?? throw new InvalidOperationException("Read OperationsGet: Missing Payload");
  }
}
public sealed class HostSettingsResource(Context context)
{
  public async Task<SettingsListResponse> List(ReadCut? at = null, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new SettingsListRequest
    {
      At = at,
    };
    var response = await context.CallAsync(HostContracts.SettingsList, request, policy, cancellationToken);
    return response;
  }
}
public sealed class HostTimeResource(Context context)
{
  public async Task<Timestamp> Get(OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new TimeGetRequest
    {
    };
    var response = await context.CallAsync(HostContracts.TimeGet, request, policy, cancellationToken);
    return response.Now ?? throw new InvalidOperationException("Read TimeGet: Missing Payload");
  }
  public async Task<TimeSleepResponse> Sleep(Duration duration, OperationPolicy? policy = null, CancellationToken cancellationToken = default)
  {
    var request = new TimeSleepRequest
    {
      Duration = duration,
    };
    var response = await context.CallAsync(HostContracts.TimeSleep, request, policy, cancellationToken);
    return response;
  }
}
