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
  public AsyncUnaryCall<global::Harmos.V1.StorageCreateFileProductionResponse> CreateFileProduction(global::Harmos.V1.StorageCreateFileProductionRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageCreateFileProductionRequest, global::Harmos.V1.StorageCreateFileProductionResponse>(MethodType.Unary, "harmos.v1.Storage", "CreateFileProduction",
        Marshallers.Create<global::Harmos.V1.StorageCreateFileProductionRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageCreateFileProductionRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageCreateFileProductionResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageCreateFileProductionResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageGetFileProductionResponse> GetFileProduction(global::Harmos.V1.StorageGetFileProductionRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageGetFileProductionRequest, global::Harmos.V1.StorageGetFileProductionResponse>(MethodType.Unary, "harmos.v1.Storage", "GetFileProduction",
        Marshallers.Create<global::Harmos.V1.StorageGetFileProductionRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetFileProductionRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageGetFileProductionResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetFileProductionResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageAppendFileBytesResponse> AppendFileBytes(global::Harmos.V1.StorageAppendFileBytesRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageAppendFileBytesRequest, global::Harmos.V1.StorageAppendFileBytesResponse>(MethodType.Unary, "harmos.v1.Storage", "AppendFileBytes",
        Marshallers.Create<global::Harmos.V1.StorageAppendFileBytesRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageAppendFileBytesRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageAppendFileBytesResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageAppendFileBytesResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageCreateFileWriterResponse> CreateFileWriter(global::Harmos.V1.StorageCreateFileWriterRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageCreateFileWriterRequest, global::Harmos.V1.StorageCreateFileWriterResponse>(MethodType.Unary, "harmos.v1.Storage", "CreateFileWriter",
        Marshallers.Create<global::Harmos.V1.StorageCreateFileWriterRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageCreateFileWriterRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageCreateFileWriterResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageCreateFileWriterResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StoragePublishFileProductionResponse> PublishFileProduction(global::Harmos.V1.StoragePublishFileProductionRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StoragePublishFileProductionRequest, global::Harmos.V1.StoragePublishFileProductionResponse>(MethodType.Unary, "harmos.v1.Storage", "PublishFileProduction",
        Marshallers.Create<global::Harmos.V1.StoragePublishFileProductionRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StoragePublishFileProductionRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StoragePublishFileProductionResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StoragePublishFileProductionResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageDeleteFileProductionResponse> DeleteFileProduction(global::Harmos.V1.StorageDeleteFileProductionRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageDeleteFileProductionRequest, global::Harmos.V1.StorageDeleteFileProductionResponse>(MethodType.Unary, "harmos.v1.Storage", "DeleteFileProduction",
        Marshallers.Create<global::Harmos.V1.StorageDeleteFileProductionRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageDeleteFileProductionRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageDeleteFileProductionResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageDeleteFileProductionResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageGetFileBytesResponse> GetFileBytes(global::Harmos.V1.StorageGetFileBytesRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageGetFileBytesRequest, global::Harmos.V1.StorageGetFileBytesResponse>(MethodType.Unary, "harmos.v1.Storage", "GetFileBytes",
        Marshallers.Create<global::Harmos.V1.StorageGetFileBytesRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetFileBytesRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageGetFileBytesResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetFileBytesResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageCapabilitiesResponse> Capabilities(global::Harmos.V1.StorageCapabilitiesRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageCapabilitiesRequest, global::Harmos.V1.StorageCapabilitiesResponse>(MethodType.Unary, "harmos.v1.Storage", "Capabilities",
        Marshallers.Create<global::Harmos.V1.StorageCapabilitiesRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageCapabilitiesRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageCapabilitiesResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageCapabilitiesResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageCreateSnapshotResponse> CreateSnapshot(global::Harmos.V1.StorageCreateSnapshotRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageCreateSnapshotRequest, global::Harmos.V1.StorageCreateSnapshotResponse>(MethodType.Unary, "harmos.v1.Storage", "CreateSnapshot",
        Marshallers.Create<global::Harmos.V1.StorageCreateSnapshotRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageCreateSnapshotRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageCreateSnapshotResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageCreateSnapshotResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageDeleteSnapshotResponse> DeleteSnapshot(global::Harmos.V1.StorageDeleteSnapshotRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageDeleteSnapshotRequest, global::Harmos.V1.StorageDeleteSnapshotResponse>(MethodType.Unary, "harmos.v1.Storage", "DeleteSnapshot",
        Marshallers.Create<global::Harmos.V1.StorageDeleteSnapshotRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageDeleteSnapshotRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageDeleteSnapshotResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageDeleteSnapshotResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageListColumnRowsResponse> ListColumnRows(global::Harmos.V1.StorageListColumnRowsRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageListColumnRowsRequest, global::Harmos.V1.StorageListColumnRowsResponse>(MethodType.Unary, "harmos.v1.Storage", "ListColumnRows",
        Marshallers.Create<global::Harmos.V1.StorageListColumnRowsRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageListColumnRowsRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageListColumnRowsResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageListColumnRowsResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageListIndexEntriesResponse> ListIndexEntries(global::Harmos.V1.StorageListIndexEntriesRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageListIndexEntriesRequest, global::Harmos.V1.StorageListIndexEntriesResponse>(MethodType.Unary, "harmos.v1.Storage", "ListIndexEntries",
        Marshallers.Create<global::Harmos.V1.StorageListIndexEntriesRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageListIndexEntriesRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageListIndexEntriesResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageListIndexEntriesResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageSetIndexDefinitionResponse> SetIndexDefinition(global::Harmos.V1.StorageSetIndexDefinitionRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageSetIndexDefinitionRequest, global::Harmos.V1.StorageSetIndexDefinitionResponse>(MethodType.Unary, "harmos.v1.Storage", "SetIndexDefinition",
        Marshallers.Create<global::Harmos.V1.StorageSetIndexDefinitionRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageSetIndexDefinitionRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageSetIndexDefinitionResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageSetIndexDefinitionResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageSetProjectionDefinitionResponse> SetProjectionDefinition(global::Harmos.V1.StorageSetProjectionDefinitionRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageSetProjectionDefinitionRequest, global::Harmos.V1.StorageSetProjectionDefinitionResponse>(MethodType.Unary, "harmos.v1.Storage", "SetProjectionDefinition",
        Marshallers.Create<global::Harmos.V1.StorageSetProjectionDefinitionRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageSetProjectionDefinitionRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageSetProjectionDefinitionResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageSetProjectionDefinitionResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageAdvanceProjectionResponse> AdvanceProjection(global::Harmos.V1.StorageAdvanceProjectionRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageAdvanceProjectionRequest, global::Harmos.V1.StorageAdvanceProjectionResponse>(MethodType.Unary, "harmos.v1.Storage", "AdvanceProjection",
        Marshallers.Create<global::Harmos.V1.StorageAdvanceProjectionRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageAdvanceProjectionRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageAdvanceProjectionResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageAdvanceProjectionResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageListProjectionRowsResponse> ListProjectionRows(global::Harmos.V1.StorageListProjectionRowsRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageListProjectionRowsRequest, global::Harmos.V1.StorageListProjectionRowsResponse>(MethodType.Unary, "harmos.v1.Storage", "ListProjectionRows",
        Marshallers.Create<global::Harmos.V1.StorageListProjectionRowsRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageListProjectionRowsRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageListProjectionRowsResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageListProjectionRowsResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageGetProjectionStatusResponse> GetProjectionStatus(global::Harmos.V1.StorageGetProjectionStatusRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageGetProjectionStatusRequest, global::Harmos.V1.StorageGetProjectionStatusResponse>(MethodType.Unary, "harmos.v1.Storage", "GetProjectionStatus",
        Marshallers.Create<global::Harmos.V1.StorageGetProjectionStatusRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetProjectionStatusRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageGetProjectionStatusResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetProjectionStatusResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageSetObjectResponse> SetObject(global::Harmos.V1.StorageSetObjectRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageSetObjectRequest, global::Harmos.V1.StorageSetObjectResponse>(MethodType.Unary, "harmos.v1.Storage", "SetObject",
        Marshallers.Create<global::Harmos.V1.StorageSetObjectRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageSetObjectRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageSetObjectResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageSetObjectResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageListRowsResponse> ListRows(global::Harmos.V1.StorageListRowsRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageListRowsRequest, global::Harmos.V1.StorageListRowsResponse>(MethodType.Unary, "harmos.v1.Storage", "ListRows",
        Marshallers.Create<global::Harmos.V1.StorageListRowsRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageListRowsRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageListRowsResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageListRowsResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.V1.StorageWatchRowsResponse> WatchRows(global::Harmos.V1.StorageWatchRowsRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageWatchRowsRequest, global::Harmos.V1.StorageWatchRowsResponse>(MethodType.ServerStreaming, "harmos.v1.Storage", "WatchRows",
        Marshallers.Create<global::Harmos.V1.StorageWatchRowsRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageWatchRowsRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageWatchRowsResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageWatchRowsResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageAppendRecordsResponse> AppendRecords(global::Harmos.V1.StorageAppendRecordsRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageAppendRecordsRequest, global::Harmos.V1.StorageAppendRecordsResponse>(MethodType.Unary, "harmos.v1.Storage", "AppendRecords",
        Marshallers.Create<global::Harmos.V1.StorageAppendRecordsRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageAppendRecordsRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageAppendRecordsResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageAppendRecordsResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageAppendSamplesResponse> AppendSamples(global::Harmos.V1.StorageAppendSamplesRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageAppendSamplesRequest, global::Harmos.V1.StorageAppendSamplesResponse>(MethodType.Unary, "harmos.v1.Storage", "AppendSamples",
        Marshallers.Create<global::Harmos.V1.StorageAppendSamplesRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageAppendSamplesRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageAppendSamplesResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageAppendSamplesResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageAppendObjectRevisionsResponse> AppendObjectRevisions(global::Harmos.V1.StorageAppendObjectRevisionsRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageAppendObjectRevisionsRequest, global::Harmos.V1.StorageAppendObjectRevisionsResponse>(MethodType.Unary, "harmos.v1.Storage", "AppendObjectRevisions",
        Marshallers.Create<global::Harmos.V1.StorageAppendObjectRevisionsRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageAppendObjectRevisionsRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageAppendObjectRevisionsResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageAppendObjectRevisionsResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageGetFileHashResponse> GetFileHash(global::Harmos.V1.StorageGetFileHashRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageGetFileHashRequest, global::Harmos.V1.StorageGetFileHashResponse>(MethodType.Unary, "harmos.v1.Storage", "GetFileHash",
        Marshallers.Create<global::Harmos.V1.StorageGetFileHashRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetFileHashRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageGetFileHashResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetFileHashResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageCaptureFileResponse> CaptureFile(global::Harmos.V1.StorageCaptureFileRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageCaptureFileRequest, global::Harmos.V1.StorageCaptureFileResponse>(MethodType.Unary, "harmos.v1.Storage", "CaptureFile",
        Marshallers.Create<global::Harmos.V1.StorageCaptureFileRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageCaptureFileRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageCaptureFileResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageCaptureFileResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.V1.StorageWatchFileBytesResponse> WatchFileBytes(global::Harmos.V1.StorageWatchFileBytesRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageWatchFileBytesRequest, global::Harmos.V1.StorageWatchFileBytesResponse>(MethodType.ServerStreaming, "harmos.v1.Storage", "WatchFileBytes",
        Marshallers.Create<global::Harmos.V1.StorageWatchFileBytesRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageWatchFileBytesRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageWatchFileBytesResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageWatchFileBytesResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.V1.StorageWatchFileProductionResponse> WatchFileProduction(global::Harmos.V1.StorageWatchFileProductionRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageWatchFileProductionRequest, global::Harmos.V1.StorageWatchFileProductionResponse>(MethodType.ServerStreaming, "harmos.v1.Storage", "WatchFileProduction",
        Marshallers.Create<global::Harmos.V1.StorageWatchFileProductionRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageWatchFileProductionRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageWatchFileProductionResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageWatchFileProductionResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageListDirectoryEntriesResponse> ListDirectoryEntries(global::Harmos.V1.StorageListDirectoryEntriesRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageListDirectoryEntriesRequest, global::Harmos.V1.StorageListDirectoryEntriesResponse>(MethodType.Unary, "harmos.v1.Storage", "ListDirectoryEntries",
        Marshallers.Create<global::Harmos.V1.StorageListDirectoryEntriesRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageListDirectoryEntriesRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageListDirectoryEntriesResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageListDirectoryEntriesResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.V1.StorageWatchDirectoryEntriesResponse> WatchDirectoryEntries(global::Harmos.V1.StorageWatchDirectoryEntriesRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageWatchDirectoryEntriesRequest, global::Harmos.V1.StorageWatchDirectoryEntriesResponse>(MethodType.ServerStreaming, "harmos.v1.Storage", "WatchDirectoryEntries",
        Marshallers.Create<global::Harmos.V1.StorageWatchDirectoryEntriesRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageWatchDirectoryEntriesRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageWatchDirectoryEntriesResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageWatchDirectoryEntriesResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageGetIndexDefinitionResponse> GetIndexDefinition(global::Harmos.V1.StorageGetIndexDefinitionRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageGetIndexDefinitionRequest, global::Harmos.V1.StorageGetIndexDefinitionResponse>(MethodType.Unary, "harmos.v1.Storage", "GetIndexDefinition",
        Marshallers.Create<global::Harmos.V1.StorageGetIndexDefinitionRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetIndexDefinitionRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageGetIndexDefinitionResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetIndexDefinitionResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageGetProjectionDefinitionResponse> GetProjectionDefinition(global::Harmos.V1.StorageGetProjectionDefinitionRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageGetProjectionDefinitionRequest, global::Harmos.V1.StorageGetProjectionDefinitionResponse>(MethodType.Unary, "harmos.v1.Storage", "GetProjectionDefinition",
        Marshallers.Create<global::Harmos.V1.StorageGetProjectionDefinitionRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetProjectionDefinitionRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageGetProjectionDefinitionResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetProjectionDefinitionResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.V1.StorageGetObjectResponse> GetObject(global::Harmos.V1.StorageGetObjectRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.V1.StorageGetObjectRequest, global::Harmos.V1.StorageGetObjectResponse>(MethodType.Unary, "harmos.v1.Storage", "GetObject",
        Marshallers.Create<global::Harmos.V1.StorageGetObjectRequest>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetObjectRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.V1.StorageGetObjectResponse>(value => value.ToByteArray(), bytes => global::Harmos.V1.StorageGetObjectResponse.Parser.ParseFrom(bytes)));
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
