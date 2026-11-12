// @generated from the protocol descriptor table; do not edit.
using Grpc.Core;
using Google.Protobuf;
namespace Harmos.Sdk;
public sealed class Client
{
  public AccessResource Access { get; }
  public CatalogEntriesResource CatalogEntries { get; }
  public InvocationsResource Invocations { get; }
  public PluginsResource Plugins { get; }
  public StatusResource Status { get; }
  public StorageResource Storage { get; }
  public TimeResource Time { get; }
  public Client(CallInvoker invoker)
  {
    Access = new AccessResource(invoker);
    CatalogEntries = new CatalogEntriesResource(invoker);
    Invocations = new InvocationsResource(invoker);
    Plugins = new PluginsResource(invoker);
    Status = new StatusResource(invoker);
    Storage = new StorageResource(invoker);
    Time = new TimeResource(invoker);
  }
}
public sealed class AccessResource
{
  public AccessCertificatesResource Certificates { get; }
  public AccessListenerResource Listener { get; }
  public AccessPairingsResource Pairings { get; }
  public AccessResource(CallInvoker invoker)
  {
    Certificates = new AccessCertificatesResource(invoker);
    Listener = new AccessListenerResource(invoker);
    Pairings = new AccessPairingsResource(invoker);
  }
}
public sealed class AccessCertificatesResource
{
  private readonly CallInvoker invoker;
  public AccessCertificatesResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.V1.Access.CertificatesCreateResponse> Create(global::Harmos.V1.Access.CertificatesCreateRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.Access.CertificatesCreateRequest, global::Harmos.V1.Access.CertificatesCreateResponse>(MethodType.Unary, "harmos.v1.access.Certificates", "Create",
        Marshallers.Create<global::Harmos.V1.Access.CertificatesCreateRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.CertificatesCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.Access.CertificatesCreateResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.CertificatesCreateResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
public sealed class AccessListenerResource
{
  private readonly CallInvoker invoker;
  public AccessListenerResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.V1.Access.ListenerGetResponse> Get(global::Harmos.V1.Access.ListenerGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.Access.ListenerGetRequest, global::Harmos.V1.Access.ListenerGetResponse>(MethodType.Unary, "harmos.v1.access.Listener", "Get",
        Marshallers.Create<global::Harmos.V1.Access.ListenerGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.ListenerGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.Access.ListenerGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.ListenerGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.Access.ListenerSetResponse> Set(global::Harmos.V1.Access.ListenerSetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.Access.ListenerSetRequest, global::Harmos.V1.Access.ListenerSetResponse>(MethodType.Unary, "harmos.v1.access.Listener", "Set",
        Marshallers.Create<global::Harmos.V1.Access.ListenerSetRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.ListenerSetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.Access.ListenerSetResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.ListenerSetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
public sealed class AccessPairingsResource
{
  private readonly CallInvoker invoker;
  public AccessPairingsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.V1.Access.PairingsCreateResponse> Create(global::Harmos.V1.Access.PairingsCreateRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.Access.PairingsCreateRequest, global::Harmos.V1.Access.PairingsCreateResponse>(MethodType.Unary, "harmos.v1.access.Pairings", "Create",
        Marshallers.Create<global::Harmos.V1.Access.PairingsCreateRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.PairingsCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.Access.PairingsCreateResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.PairingsCreateResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.Access.PairingsListResponse> List(global::Harmos.V1.Access.PairingsListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.Access.PairingsListRequest, global::Harmos.V1.Access.PairingsListResponse>(MethodType.Unary, "harmos.v1.access.Pairings", "List",
        Marshallers.Create<global::Harmos.V1.Access.PairingsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.PairingsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.Access.PairingsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.PairingsListResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.Access.PairingsDeleteResponse> Delete(global::Harmos.V1.Access.PairingsDeleteRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.Access.PairingsDeleteRequest, global::Harmos.V1.Access.PairingsDeleteResponse>(MethodType.Unary, "harmos.v1.access.Pairings", "Delete",
        Marshallers.Create<global::Harmos.V1.Access.PairingsDeleteRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.PairingsDeleteRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.Access.PairingsDeleteResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.Access.PairingsDeleteResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
public sealed class CatalogEntriesResource
{
  private readonly CallInvoker invoker;
  public CatalogEntriesResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.V1.CatalogEntriesListResponse> List(global::Harmos.V1.CatalogEntriesListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.CatalogEntriesListRequest, global::Harmos.V1.CatalogEntriesListResponse>(MethodType.Unary, "harmos.v1.CatalogEntries", "List",
        Marshallers.Create<global::Harmos.V1.CatalogEntriesListRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.CatalogEntriesListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.CatalogEntriesListResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.CatalogEntriesListResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
public sealed class InvocationsResource
{
  private readonly CallInvoker invoker;
  public InvocationsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.V1.InvocationsInvokeResponse> Invoke(global::Harmos.V1.InvocationsInvokeRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.InvocationsInvokeRequest, global::Harmos.V1.InvocationsInvokeResponse>(MethodType.Unary, "harmos.v1.Invocations", "Invoke",
        Marshallers.Create<global::Harmos.V1.InvocationsInvokeRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.InvocationsInvokeRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.InvocationsInvokeResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.InvocationsInvokeResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.InvocationsGetResponse> Get(global::Harmos.V1.InvocationsGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.InvocationsGetRequest, global::Harmos.V1.InvocationsGetResponse>(MethodType.Unary, "harmos.v1.Invocations", "Get",
        Marshallers.Create<global::Harmos.V1.InvocationsGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.InvocationsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.InvocationsGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.InvocationsGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.InvocationsCancelResponse> Cancel(global::Harmos.V1.InvocationsCancelRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.InvocationsCancelRequest, global::Harmos.V1.InvocationsCancelResponse>(MethodType.Unary, "harmos.v1.Invocations", "Cancel",
        Marshallers.Create<global::Harmos.V1.InvocationsCancelRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.InvocationsCancelRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.InvocationsCancelResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.InvocationsCancelResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.V1.InvocationsWatchResponse> Watch(global::Harmos.V1.InvocationsWatchRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.InvocationsWatchRequest, global::Harmos.V1.InvocationsWatchResponse>(MethodType.ServerStreaming, "harmos.v1.Invocations", "Watch",
        Marshallers.Create<global::Harmos.V1.InvocationsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.InvocationsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.InvocationsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.InvocationsWatchResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
}
public sealed class PluginsResource
{
  private readonly CallInvoker invoker;
  public PluginsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.V1.PluginsGetResponse> Get(global::Harmos.V1.PluginsGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.PluginsGetRequest, global::Harmos.V1.PluginsGetResponse>(MethodType.Unary, "harmos.v1.Plugins", "Get",
        Marshallers.Create<global::Harmos.V1.PluginsGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.PluginsGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.PluginsListResponse> List(global::Harmos.V1.PluginsListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.PluginsListRequest, global::Harmos.V1.PluginsListResponse>(MethodType.Unary, "harmos.v1.Plugins", "List",
        Marshallers.Create<global::Harmos.V1.PluginsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.PluginsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsListResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.V1.PluginsWatchResponse> Watch(global::Harmos.V1.PluginsWatchRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.PluginsWatchRequest, global::Harmos.V1.PluginsWatchResponse>(MethodType.ServerStreaming, "harmos.v1.Plugins", "Watch",
        Marshallers.Create<global::Harmos.V1.PluginsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.PluginsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsWatchResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.PluginsCreateResponse> Create(global::Harmos.V1.PluginsCreateRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.PluginsCreateRequest, global::Harmos.V1.PluginsCreateResponse>(MethodType.Unary, "harmos.v1.Plugins", "Create",
        Marshallers.Create<global::Harmos.V1.PluginsCreateRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.PluginsCreateResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsCreateResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.PluginsSetResponse> Set(global::Harmos.V1.PluginsSetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.PluginsSetRequest, global::Harmos.V1.PluginsSetResponse>(MethodType.Unary, "harmos.v1.Plugins", "Set",
        Marshallers.Create<global::Harmos.V1.PluginsSetRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsSetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.PluginsSetResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsSetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.PluginsDeleteResponse> Delete(global::Harmos.V1.PluginsDeleteRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.PluginsDeleteRequest, global::Harmos.V1.PluginsDeleteResponse>(MethodType.Unary, "harmos.v1.Plugins", "Delete",
        Marshallers.Create<global::Harmos.V1.PluginsDeleteRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsDeleteRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.PluginsDeleteResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsDeleteResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.PluginsRestartResponse> Restart(global::Harmos.V1.PluginsRestartRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.PluginsRestartRequest, global::Harmos.V1.PluginsRestartResponse>(MethodType.Unary, "harmos.v1.Plugins", "Restart",
        Marshallers.Create<global::Harmos.V1.PluginsRestartRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsRestartRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.PluginsRestartResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.PluginsRestartResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
public sealed class StatusResource
{
  private readonly CallInvoker invoker;
  public StatusDiagnosticsResource Diagnostics { get; }
  public StatusLogsResource Logs { get; }
  public StatusTraceEventsResource TraceEvents { get; }
  public StatusResource(CallInvoker invoker)
  {
    this.invoker = invoker;
    Diagnostics = new StatusDiagnosticsResource(invoker);
    Logs = new StatusLogsResource(invoker);
    TraceEvents = new StatusTraceEventsResource(invoker);
  }
  public AsyncUnaryCall<global::Harmos.V1.StatusGetResponse> Get(global::Harmos.V1.StatusGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StatusGetRequest, global::Harmos.V1.StatusGetResponse>(MethodType.Unary, "harmos.v1.Status", "Get",
        Marshallers.Create<global::Harmos.V1.StatusGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StatusGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StatusGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StatusGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.V1.StatusWatchResponse> Watch(global::Harmos.V1.StatusWatchRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StatusWatchRequest, global::Harmos.V1.StatusWatchResponse>(MethodType.ServerStreaming, "harmos.v1.Status", "Watch",
        Marshallers.Create<global::Harmos.V1.StatusWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StatusWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StatusWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StatusWatchResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
}
public sealed class StatusDiagnosticsResource
{
  private readonly CallInvoker invoker;
  public StatusDiagnosticsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.V1.Status.DiagnosticsGetResponse> Get(global::Harmos.V1.Status.DiagnosticsGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.Status.DiagnosticsGetRequest, global::Harmos.V1.Status.DiagnosticsGetResponse>(MethodType.Unary, "harmos.v1.status.Diagnostics", "Get",
        Marshallers.Create<global::Harmos.V1.Status.DiagnosticsGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.Status.DiagnosticsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.Status.DiagnosticsGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.Status.DiagnosticsGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
public sealed class StatusLogsResource
{
  private readonly CallInvoker invoker;
  public StatusLogsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.V1.Status.LogsListResponse> List(global::Harmos.V1.Status.LogsListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.Status.LogsListRequest, global::Harmos.V1.Status.LogsListResponse>(MethodType.Unary, "harmos.v1.status.Logs", "List",
        Marshallers.Create<global::Harmos.V1.Status.LogsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.Status.LogsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.Status.LogsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.Status.LogsListResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.V1.Status.LogsWatchResponse> Watch(global::Harmos.V1.Status.LogsWatchRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.Status.LogsWatchRequest, global::Harmos.V1.Status.LogsWatchResponse>(MethodType.ServerStreaming, "harmos.v1.status.Logs", "Watch",
        Marshallers.Create<global::Harmos.V1.Status.LogsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.Status.LogsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.Status.LogsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.Status.LogsWatchResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
}
public sealed class StatusTraceEventsResource
{
  private readonly CallInvoker invoker;
  public StatusTraceEventsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.V1.Status.TraceEventsListResponse> List(global::Harmos.V1.Status.TraceEventsListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.Status.TraceEventsListRequest, global::Harmos.V1.Status.TraceEventsListResponse>(MethodType.Unary, "harmos.v1.status.TraceEvents", "List",
        Marshallers.Create<global::Harmos.V1.Status.TraceEventsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.Status.TraceEventsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.Status.TraceEventsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.Status.TraceEventsListResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.V1.Status.TraceEventsWatchResponse> Watch(global::Harmos.V1.Status.TraceEventsWatchRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.Status.TraceEventsWatchRequest, global::Harmos.V1.Status.TraceEventsWatchResponse>(MethodType.ServerStreaming, "harmos.v1.status.TraceEvents", "Watch",
        Marshallers.Create<global::Harmos.V1.Status.TraceEventsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.Status.TraceEventsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.Status.TraceEventsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.Status.TraceEventsWatchResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
}
public sealed class StorageResource
{
  private readonly CallInvoker invoker;
  public StorageResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.Storage.V1.StorageCapabilities> Capabilities(global::Harmos.Storage.V1.Capabilities request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Storage.V1.Capabilities, global::Harmos.Storage.V1.StorageCapabilities>(MethodType.Unary, "harmos.storage.v1.Storage", "Capabilities",
        Marshallers.Create<global::Harmos.Storage.V1.Capabilities>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.Capabilities.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Storage.V1.StorageCapabilities>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.StorageCapabilities.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Storage.V1.SavedSnapshot> OpenSnapshot(global::Harmos.Storage.V1.OpenSnapshot request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Storage.V1.OpenSnapshot, global::Harmos.Storage.V1.SavedSnapshot>(MethodType.Unary, "harmos.storage.v1.Storage", "OpenSnapshot",
        Marshallers.Create<global::Harmos.Storage.V1.OpenSnapshot>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.OpenSnapshot.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Storage.V1.SavedSnapshot>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.SavedSnapshot.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Storage.V1.Empty> ReleaseSnapshot(global::Harmos.Storage.V1.ReleaseSnapshot request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Storage.V1.ReleaseSnapshot, global::Harmos.Storage.V1.Empty>(MethodType.Unary, "harmos.storage.v1.Storage", "ReleaseSnapshot",
        Marshallers.Create<global::Harmos.Storage.V1.ReleaseSnapshot>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.ReleaseSnapshot.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Storage.V1.Empty>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.Empty.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Storage.V1.ArrowPage> ScanArrow(global::Harmos.Storage.V1.ScanArrow request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Storage.V1.ScanArrow, global::Harmos.Storage.V1.ArrowPage>(MethodType.Unary, "harmos.storage.v1.Storage", "ScanArrow",
        Marshallers.Create<global::Harmos.Storage.V1.ScanArrow>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.ScanArrow.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Storage.V1.ArrowPage>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.ArrowPage.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Storage.V1.IndexPage> ScanIndex(global::Harmos.Storage.V1.ScanIndex request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Storage.V1.ScanIndex, global::Harmos.Storage.V1.IndexPage>(MethodType.Unary, "harmos.storage.v1.Storage", "ScanIndex",
        Marshallers.Create<global::Harmos.Storage.V1.ScanIndex>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.ScanIndex.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Storage.V1.IndexPage>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.IndexPage.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Storage.V1.DefinitionRevision> RegisterIndex(global::Harmos.Storage.V1.RegisterIndex request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Storage.V1.RegisterIndex, global::Harmos.Storage.V1.DefinitionRevision>(MethodType.Unary, "harmos.storage.v1.Storage", "RegisterIndex",
        Marshallers.Create<global::Harmos.Storage.V1.RegisterIndex>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.RegisterIndex.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Storage.V1.DefinitionRevision>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.DefinitionRevision.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Storage.V1.DefinitionRevision> RegisterProjection(global::Harmos.Storage.V1.RegisterProjection request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Storage.V1.RegisterProjection, global::Harmos.Storage.V1.DefinitionRevision>(MethodType.Unary, "harmos.storage.v1.Storage", "RegisterProjection",
        Marshallers.Create<global::Harmos.Storage.V1.RegisterProjection>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.RegisterProjection.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Storage.V1.DefinitionRevision>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.DefinitionRevision.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Storage.V1.DefinitionRevision> AdvanceProjection(global::Harmos.Storage.V1.AdvanceProjection request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Storage.V1.AdvanceProjection, global::Harmos.Storage.V1.DefinitionRevision>(MethodType.Unary, "harmos.storage.v1.Storage", "AdvanceProjection",
        Marshallers.Create<global::Harmos.Storage.V1.AdvanceProjection>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.AdvanceProjection.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Storage.V1.DefinitionRevision>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.DefinitionRevision.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Storage.V1.ProjectionPage> ReadProjection(global::Harmos.Storage.V1.ReadProjection request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Storage.V1.ReadProjection, global::Harmos.Storage.V1.ProjectionPage>(MethodType.Unary, "harmos.storage.v1.Storage", "ReadProjection",
        Marshallers.Create<global::Harmos.Storage.V1.ReadProjection>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.ReadProjection.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Storage.V1.ProjectionPage>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.ProjectionPage.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Storage.V1.ProjectionState> InspectProjection(global::Harmos.Storage.V1.InspectProjection request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Storage.V1.InspectProjection, global::Harmos.Storage.V1.ProjectionState>(MethodType.Unary, "harmos.storage.v1.Storage", "InspectProjection",
        Marshallers.Create<global::Harmos.Storage.V1.InspectProjection>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.InspectProjection.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Storage.V1.ProjectionState>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.ProjectionState.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Storage.V1.CompareExchangeOutcome> CompareExchangeObject(global::Harmos.Storage.V1.CompareExchangeObject request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Storage.V1.CompareExchangeObject, global::Harmos.Storage.V1.CompareExchangeOutcome>(MethodType.Unary, "harmos.storage.v1.Storage", "CompareExchangeObject",
        Marshallers.Create<global::Harmos.Storage.V1.CompareExchangeObject>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.CompareExchangeObject.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Storage.V1.CompareExchangeOutcome>(value => value.ToByteArray(), bytes => global::Harmos.Storage.V1.CompareExchangeOutcome.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
public sealed class TimeResource
{
  private readonly CallInvoker invoker;
  public TimeResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.V1.TimeGetResponse> Get(global::Harmos.V1.TimeGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.TimeGetRequest, global::Harmos.V1.TimeGetResponse>(MethodType.Unary, "harmos.v1.Time", "Get",
        Marshallers.Create<global::Harmos.V1.TimeGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.TimeGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.TimeGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.TimeGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.TimeSleepResponse> Sleep(global::Harmos.V1.TimeSleepRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.TimeSleepRequest, global::Harmos.V1.TimeSleepResponse>(MethodType.Unary, "harmos.v1.Time", "Sleep",
        Marshallers.Create<global::Harmos.V1.TimeSleepRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.TimeSleepRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.TimeSleepResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.TimeSleepResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
