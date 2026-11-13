// @generated from the protocol descriptor table; do not edit.
#nullable enable
using Grpc.Core;
using Google.Protobuf;
namespace Harmos.Sdk;

public sealed class Client : global::System.IDisposable
{
  private global::Grpc.Net.Client.GrpcChannel? ownedChannel;
  public AccessResource Access { get; }
  public InvocationsResource Invocations { get; }
  public OperationsResource Operations { get; }
  public PluginsResource Plugins { get; }
  public SchemasResource Schemas { get; }
  public StatusResource Status { get; }
  public StreamsResource Streams { get; }
  public TimeResource Time { get; }
  public Client(CallInvoker invoker)
  {
    Access = new AccessResource(invoker);
    Invocations = new InvocationsResource(invoker);
    Operations = new OperationsResource(invoker);
    Plugins = new PluginsResource(invoker);
    Schemas = new SchemasResource(invoker);
    Status = new StatusResource(invoker);
    Streams = new StreamsResource(invoker);
    Time = new TimeResource(invoker);
  }
  public static Client Connect(string address, global::Harmos.Storage.V1.StorageTransferSettings? transfer = null, int? messageBytes = null)
  {
    var maximum = Transfer.MessageBytes(transfer, messageBytes);
    var channel = global::Grpc.Net.Client.GrpcChannel.ForAddress(address, new() { MaxSendMessageSize = maximum, MaxReceiveMessageSize = maximum });
    return new Client(channel.CreateCallInvoker()) { ownedChannel = channel };
  }
  public void Dispose() => ownedChannel?.Dispose();
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
  public AsyncUnaryCall<global::Harmos.Client.V1.AccessResources.CertificatesCreateResponse> Create(global::Harmos.Client.V1.AccessResources.CertificatesCreateRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.AccessResources.CertificatesCreateRequest, global::Harmos.Client.V1.AccessResources.CertificatesCreateResponse>(MethodType.Unary, "harmos.client.v1.access.Certificates", "Create",
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.CertificatesCreateRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.CertificatesCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.CertificatesCreateResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.CertificatesCreateResponse.Parser.ParseFrom(bytes)));
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
  public AsyncUnaryCall<global::Harmos.Client.V1.AccessResources.ListenerGetResponse> Get(global::Harmos.Client.V1.AccessResources.ListenerGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.AccessResources.ListenerGetRequest, global::Harmos.Client.V1.AccessResources.ListenerGetResponse>(MethodType.Unary, "harmos.client.v1.access.Listener", "Get",
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.ListenerGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.ListenerGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.ListenerGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.ListenerGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.AccessResources.ListenerSetResponse> Set(global::Harmos.Client.V1.AccessResources.ListenerSetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.AccessResources.ListenerSetRequest, global::Harmos.Client.V1.AccessResources.ListenerSetResponse>(MethodType.Unary, "harmos.client.v1.access.Listener", "Set",
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.ListenerSetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.ListenerSetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.ListenerSetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.ListenerSetResponse.Parser.ParseFrom(bytes)));
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
  public AsyncUnaryCall<global::Harmos.Client.V1.AccessResources.PairingsCreateResponse> Create(global::Harmos.Client.V1.AccessResources.PairingsCreateRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.AccessResources.PairingsCreateRequest, global::Harmos.Client.V1.AccessResources.PairingsCreateResponse>(MethodType.Unary, "harmos.client.v1.access.Pairings", "Create",
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.PairingsCreateRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.PairingsCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.PairingsCreateResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.PairingsCreateResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.AccessResources.PairingsListResponse> List(global::Harmos.Client.V1.AccessResources.PairingsListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.AccessResources.PairingsListRequest, global::Harmos.Client.V1.AccessResources.PairingsListResponse>(MethodType.Unary, "harmos.client.v1.access.Pairings", "List",
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.PairingsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.PairingsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.PairingsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.PairingsListResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.AccessResources.PairingsDeleteResponse> Delete(global::Harmos.Client.V1.AccessResources.PairingsDeleteRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.AccessResources.PairingsDeleteRequest, global::Harmos.Client.V1.AccessResources.PairingsDeleteResponse>(MethodType.Unary, "harmos.client.v1.access.Pairings", "Delete",
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.PairingsDeleteRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.PairingsDeleteRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.AccessResources.PairingsDeleteResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.AccessResources.PairingsDeleteResponse.Parser.ParseFrom(bytes)));
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
  public AsyncUnaryCall<global::Harmos.Client.V1.InvocationsInvokeResponse> Invoke(global::Harmos.Client.V1.InvocationsInvokeRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.InvocationsInvokeRequest, global::Harmos.Client.V1.InvocationsInvokeResponse>(MethodType.Unary, "harmos.client.v1.Invocations", "Invoke",
        Marshallers.Create<global::Harmos.Client.V1.InvocationsInvokeRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.InvocationsInvokeRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.InvocationsInvokeResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.InvocationsInvokeResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.InvocationsGetResponse> Get(global::Harmos.Client.V1.InvocationsGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.InvocationsGetRequest, global::Harmos.Client.V1.InvocationsGetResponse>(MethodType.Unary, "harmos.client.v1.Invocations", "Get",
        Marshallers.Create<global::Harmos.Client.V1.InvocationsGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.InvocationsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.InvocationsGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.InvocationsGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.InvocationsCancelResponse> Cancel(global::Harmos.Client.V1.InvocationsCancelRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.InvocationsCancelRequest, global::Harmos.Client.V1.InvocationsCancelResponse>(MethodType.Unary, "harmos.client.v1.Invocations", "Cancel",
        Marshallers.Create<global::Harmos.Client.V1.InvocationsCancelRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.InvocationsCancelRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.InvocationsCancelResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.InvocationsCancelResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.Client.V1.InvocationsWatchResponse> Watch(global::Harmos.Client.V1.InvocationsWatchRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.InvocationsWatchRequest, global::Harmos.Client.V1.InvocationsWatchResponse>(MethodType.ServerStreaming, "harmos.client.v1.Invocations", "Watch",
        Marshallers.Create<global::Harmos.Client.V1.InvocationsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.InvocationsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.InvocationsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.InvocationsWatchResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
}
public sealed class OperationsResource
{
  private readonly CallInvoker invoker;
  public OperationsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.OperationsListResponse> List(global::Harmos.Client.V1.OperationsListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.OperationsListRequest, global::Harmos.Client.V1.OperationsListResponse>(MethodType.Unary, "harmos.client.v1.Operations", "List",
        Marshallers.Create<global::Harmos.Client.V1.OperationsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.OperationsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.OperationsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.OperationsListResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.OperationsGetResponse> Get(global::Harmos.Client.V1.OperationsGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.OperationsGetRequest, global::Harmos.Client.V1.OperationsGetResponse>(MethodType.Unary, "harmos.client.v1.Operations", "Get",
        Marshallers.Create<global::Harmos.Client.V1.OperationsGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.OperationsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.OperationsGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.OperationsGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
public sealed class PluginsResource
{
  private readonly CallInvoker invoker;
  public PluginsSettingsResource Settings { get; }
  public PluginsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
    Settings = new PluginsSettingsResource(invoker);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.PluginsGetResponse> Get(global::Harmos.Client.V1.PluginsGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.PluginsGetRequest, global::Harmos.Client.V1.PluginsGetResponse>(MethodType.Unary, "harmos.client.v1.Plugins", "Get",
        Marshallers.Create<global::Harmos.Client.V1.PluginsGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.PluginsGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.PluginsListResponse> List(global::Harmos.Client.V1.PluginsListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.PluginsListRequest, global::Harmos.Client.V1.PluginsListResponse>(MethodType.Unary, "harmos.client.v1.Plugins", "List",
        Marshallers.Create<global::Harmos.Client.V1.PluginsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.PluginsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsListResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.Client.V1.PluginsWatchResponse> Watch(global::Harmos.Client.V1.PluginsWatchRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.PluginsWatchRequest, global::Harmos.Client.V1.PluginsWatchResponse>(MethodType.ServerStreaming, "harmos.client.v1.Plugins", "Watch",
        Marshallers.Create<global::Harmos.Client.V1.PluginsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.PluginsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsWatchResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.PluginsCreateResponse> Create(global::Harmos.Client.V1.PluginsCreateRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.PluginsCreateRequest, global::Harmos.Client.V1.PluginsCreateResponse>(MethodType.Unary, "harmos.client.v1.Plugins", "Create",
        Marshallers.Create<global::Harmos.Client.V1.PluginsCreateRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.PluginsCreateResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsCreateResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.PluginsSetResponse> Set(global::Harmos.Client.V1.PluginsSetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.PluginsSetRequest, global::Harmos.Client.V1.PluginsSetResponse>(MethodType.Unary, "harmos.client.v1.Plugins", "Set",
        Marshallers.Create<global::Harmos.Client.V1.PluginsSetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsSetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.PluginsSetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsSetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.PluginsDeleteResponse> Delete(global::Harmos.Client.V1.PluginsDeleteRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.PluginsDeleteRequest, global::Harmos.Client.V1.PluginsDeleteResponse>(MethodType.Unary, "harmos.client.v1.Plugins", "Delete",
        Marshallers.Create<global::Harmos.Client.V1.PluginsDeleteRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsDeleteRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.PluginsDeleteResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsDeleteResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.PluginsRestartResponse> Restart(global::Harmos.Client.V1.PluginsRestartRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.PluginsRestartRequest, global::Harmos.Client.V1.PluginsRestartResponse>(MethodType.Unary, "harmos.client.v1.Plugins", "Restart",
        Marshallers.Create<global::Harmos.Client.V1.PluginsRestartRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsRestartRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.PluginsRestartResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsRestartResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.PluginsReplaceResponse> Replace(global::Harmos.Client.V1.PluginsReplaceRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.PluginsReplaceRequest, global::Harmos.Client.V1.PluginsReplaceResponse>(MethodType.Unary, "harmos.client.v1.Plugins", "Replace",
        Marshallers.Create<global::Harmos.Client.V1.PluginsReplaceRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsReplaceRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.PluginsReplaceResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsReplaceResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
public sealed class PluginsSettingsResource
{
  private readonly CallInvoker invoker;
  public PluginsSettingsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.PluginsResources.SettingsListResponse> List(global::Harmos.Client.V1.PluginsResources.SettingsListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.PluginsResources.SettingsListRequest, global::Harmos.Client.V1.PluginsResources.SettingsListResponse>(MethodType.Unary, "harmos.client.v1.plugins.Settings", "List",
        Marshallers.Create<global::Harmos.Client.V1.PluginsResources.SettingsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsResources.SettingsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.PluginsResources.SettingsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsResources.SettingsListResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.PluginsResources.SettingsSetResponse> Set(global::Harmos.Client.V1.PluginsResources.SettingsSetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.PluginsResources.SettingsSetRequest, global::Harmos.Client.V1.PluginsResources.SettingsSetResponse>(MethodType.Unary, "harmos.client.v1.plugins.Settings", "Set",
        Marshallers.Create<global::Harmos.Client.V1.PluginsResources.SettingsSetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsResources.SettingsSetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.PluginsResources.SettingsSetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.PluginsResources.SettingsSetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
public sealed class SchemasResource
{
  private readonly CallInvoker invoker;
  public SchemasResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.SchemasListResponse> List(global::Harmos.Client.V1.SchemasListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.SchemasListRequest, global::Harmos.Client.V1.SchemasListResponse>(MethodType.Unary, "harmos.client.v1.Schemas", "List",
        Marshallers.Create<global::Harmos.Client.V1.SchemasListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.SchemasListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.SchemasListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.SchemasListResponse.Parser.ParseFrom(bytes)));
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
  public AsyncUnaryCall<global::Harmos.Client.V1.StatusGetResponse> Get(global::Harmos.Client.V1.StatusGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.StatusGetRequest, global::Harmos.Client.V1.StatusGetResponse>(MethodType.Unary, "harmos.client.v1.Status", "Get",
        Marshallers.Create<global::Harmos.Client.V1.StatusGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.StatusGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.Client.V1.StatusWatchResponse> Watch(global::Harmos.Client.V1.StatusWatchRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.StatusWatchRequest, global::Harmos.Client.V1.StatusWatchResponse>(MethodType.ServerStreaming, "harmos.client.v1.Status", "Watch",
        Marshallers.Create<global::Harmos.Client.V1.StatusWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.StatusWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusWatchResponse.Parser.ParseFrom(bytes)));
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
  public AsyncUnaryCall<global::Harmos.Client.V1.StatusResources.DiagnosticsGetResponse> Get(global::Harmos.Client.V1.StatusResources.DiagnosticsGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.StatusResources.DiagnosticsGetRequest, global::Harmos.Client.V1.StatusResources.DiagnosticsGetResponse>(MethodType.Unary, "harmos.client.v1.status.Diagnostics", "Get",
        Marshallers.Create<global::Harmos.Client.V1.StatusResources.DiagnosticsGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusResources.DiagnosticsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.StatusResources.DiagnosticsGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusResources.DiagnosticsGetResponse.Parser.ParseFrom(bytes)));
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
  public AsyncUnaryCall<global::Harmos.Client.V1.StatusResources.LogsListResponse> List(global::Harmos.Client.V1.StatusResources.LogsListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.StatusResources.LogsListRequest, global::Harmos.Client.V1.StatusResources.LogsListResponse>(MethodType.Unary, "harmos.client.v1.status.Logs", "List",
        Marshallers.Create<global::Harmos.Client.V1.StatusResources.LogsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusResources.LogsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.StatusResources.LogsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusResources.LogsListResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.Client.V1.StatusResources.LogsWatchResponse> Watch(global::Harmos.Client.V1.StatusResources.LogsWatchRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.StatusResources.LogsWatchRequest, global::Harmos.Client.V1.StatusResources.LogsWatchResponse>(MethodType.ServerStreaming, "harmos.client.v1.status.Logs", "Watch",
        Marshallers.Create<global::Harmos.Client.V1.StatusResources.LogsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusResources.LogsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.StatusResources.LogsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusResources.LogsWatchResponse.Parser.ParseFrom(bytes)));
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
  public AsyncUnaryCall<global::Harmos.Client.V1.StatusResources.TraceEventsListResponse> List(global::Harmos.Client.V1.StatusResources.TraceEventsListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.StatusResources.TraceEventsListRequest, global::Harmos.Client.V1.StatusResources.TraceEventsListResponse>(MethodType.Unary, "harmos.client.v1.status.TraceEvents", "List",
        Marshallers.Create<global::Harmos.Client.V1.StatusResources.TraceEventsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusResources.TraceEventsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.StatusResources.TraceEventsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusResources.TraceEventsListResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncServerStreamingCall<global::Harmos.Client.V1.StatusResources.TraceEventsWatchResponse> Watch(global::Harmos.Client.V1.StatusResources.TraceEventsWatchRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.StatusResources.TraceEventsWatchRequest, global::Harmos.Client.V1.StatusResources.TraceEventsWatchResponse>(MethodType.ServerStreaming, "harmos.client.v1.status.TraceEvents", "Watch",
        Marshallers.Create<global::Harmos.Client.V1.StatusResources.TraceEventsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusResources.TraceEventsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.StatusResources.TraceEventsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StatusResources.TraceEventsWatchResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
  }
}
public sealed class StreamsResource
{
  private readonly CallInvoker invoker;
  public StreamsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.StreamsListResponse> List(global::Harmos.Client.V1.StreamsListRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.StreamsListRequest, global::Harmos.Client.V1.StreamsListResponse>(MethodType.Unary, "harmos.client.v1.Streams", "List",
        Marshallers.Create<global::Harmos.Client.V1.StreamsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StreamsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.StreamsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.StreamsListResponse.Parser.ParseFrom(bytes)));
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
  public AsyncUnaryCall<global::Harmos.Client.V1.TimeGetResponse> Get(global::Harmos.Client.V1.TimeGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.TimeGetRequest, global::Harmos.Client.V1.TimeGetResponse>(MethodType.Unary, "harmos.client.v1.Time", "Get",
        Marshallers.Create<global::Harmos.Client.V1.TimeGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.TimeGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.TimeGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.TimeGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
  public AsyncUnaryCall<global::Harmos.Client.V1.TimeSleepResponse> Sleep(global::Harmos.Client.V1.TimeSleepRequest request, CallOptions options = default)
  {
    var method = new Method<global::Harmos.Client.V1.TimeSleepRequest, global::Harmos.Client.V1.TimeSleepResponse>(MethodType.Unary, "harmos.client.v1.Time", "Sleep",
        Marshallers.Create<global::Harmos.Client.V1.TimeSleepRequest>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.TimeSleepRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Client.V1.TimeSleepResponse>(value => value.ToByteArray(), bytes => global::Harmos.Client.V1.TimeSleepResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
