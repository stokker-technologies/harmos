// @generated from protocol descriptors; do not edit.
#nullable enable
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Grpc.Net.Client;
using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
namespace Fixture;

public sealed class Client : IDisposable
{
  private GrpcChannel? ownedChannel;
  public AccessResource Access { get; }
  public InvocationsResource Invocations { get; }
  public JobsResource Jobs { get; }
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
    Jobs = new JobsResource(invoker);
    Operations = new OperationsResource(invoker);
    Plugins = new PluginsResource(invoker);
    Schemas = new SchemasResource(invoker);
    Status = new StatusResource(invoker);
    Streams = new StreamsResource(invoker);
    Time = new TimeResource(invoker);
  }
  public static Client Connect(string address, global::Harmos.StorageTransferSettings? transfer = null, int? messageBytes = null)
  {
    var maximum = global::Harmos.Transfer.MessageBytes(transfer, messageBytes);
    var channel = GrpcChannel.ForAddress(address, new() { MaxSendMessageSize = maximum, MaxReceiveMessageSize = maximum });
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
  public async Task<string> Create(CallOptions context = default)
  {
    var request = new global::Harmos.Clients.CertificatesCreateRequest
    {
    };
    var method = new Method<global::Harmos.Clients.CertificatesCreateRequest, global::Harmos.Clients.CertificatesCreateResponse>(MethodType.Unary, "application.host.v1.access.Certificates", "Create",
        Marshallers.Create<global::Harmos.Clients.CertificatesCreateRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.CertificatesCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.CertificatesCreateResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.CertificatesCreateResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.CertificatesCreateResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call access.certificates.create: " + error.Status.Detail), error.Trailers);
    }
    return response.CertificatePem;
  }
}
public sealed class AccessListenerResource
{
  private readonly CallInvoker invoker;
  public AccessListenerResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<global::Harmos.Clients.ListenerGetResponse> Get(CallOptions context = default)
  {
    var request = new global::Harmos.Clients.ListenerGetRequest
    {
    };
    var method = new Method<global::Harmos.Clients.ListenerGetRequest, global::Harmos.Clients.ListenerGetResponse>(MethodType.Unary, "application.host.v1.access.Listener", "Get",
        Marshallers.Create<global::Harmos.Clients.ListenerGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.ListenerGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.ListenerGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.ListenerGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.ListenerGetResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call access.listener.get: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
  public async Task<global::Harmos.Clients.ListenerSetResponse> Set(global::Harmos.Clients.ListenerValue value, ByteString expectedRevision, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.ListenerSetRequest
    {
      Value = value,
      ExpectedRevision = expectedRevision,
    };
    var method = new Method<global::Harmos.Clients.ListenerSetRequest, global::Harmos.Clients.ListenerSetResponse>(MethodType.Unary, "application.host.v1.access.Listener", "Set",
        Marshallers.Create<global::Harmos.Clients.ListenerSetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.ListenerSetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.ListenerSetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.ListenerSetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.ListenerSetResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call access.listener.set: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
}
public sealed class AccessPairingsResource
{
  private readonly CallInvoker invoker;
  public AccessPairingsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<global::Harmos.Clients.PairingsCreateResponse> Create(string label, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.PairingsCreateRequest
    {
      Label = label,
    };
    var method = new Method<global::Harmos.Clients.PairingsCreateRequest, global::Harmos.Clients.PairingsCreateResponse>(MethodType.Unary, "application.host.v1.access.Pairings", "Create",
        Marshallers.Create<global::Harmos.Clients.PairingsCreateRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PairingsCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.PairingsCreateResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PairingsCreateResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.PairingsCreateResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call access.pairings.create: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
  public async Task<global::Harmos.Clients.PairingsListResponse> List(global::Harmos.Position? at = null, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.PairingsListRequest
    {
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<global::Harmos.Clients.PairingsListRequest, global::Harmos.Clients.PairingsListResponse>(MethodType.Unary, "application.host.v1.access.Pairings", "List",
        Marshallers.Create<global::Harmos.Clients.PairingsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PairingsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.PairingsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PairingsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.PairingsListResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call access.pairings.list: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
  public async Task<global::Harmos.Clients.PairingsDeleteResponse> Delete(string name, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.PairingsDeleteRequest
    {
      Name = name,
    };
    var method = new Method<global::Harmos.Clients.PairingsDeleteRequest, global::Harmos.Clients.PairingsDeleteResponse>(MethodType.Unary, "application.host.v1.access.Pairings", "Delete",
        Marshallers.Create<global::Harmos.Clients.PairingsDeleteRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PairingsDeleteRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.PairingsDeleteResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PairingsDeleteResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.PairingsDeleteResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call access.pairings.delete: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
}
public sealed class InvocationsResource
{
  private readonly CallInvoker invoker;
  public InvocationsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<ByteString> Create(string operation, ByteString fingerprint, ByteString payload, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.InvocationsCreateRequest
    {
      Operation = operation,
      Fingerprint = fingerprint,
      Payload = payload,
    };
    var method = new Method<global::Harmos.Clients.InvocationsCreateRequest, global::Harmos.Clients.InvocationsCreateResponse>(MethodType.Unary, "application.host.v1.Invocations", "Create",
        Marshallers.Create<global::Harmos.Clients.InvocationsCreateRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.InvocationsCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.InvocationsCreateResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.InvocationsCreateResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.InvocationsCreateResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call invocations.create: " + error.Status.Detail), error.Trailers);
    }
    return response.Payload;
  }
  public async Task<global::Harmos.InstanceState> Get(ulong instance, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.InvocationsGetRequest
    {
      Instance = instance,
    };
    var method = new Method<global::Harmos.Clients.InvocationsGetRequest, global::Harmos.Clients.InvocationsGetResponse>(MethodType.Unary, "application.host.v1.Invocations", "Get",
        Marshallers.Create<global::Harmos.Clients.InvocationsGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.InvocationsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.InvocationsGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.InvocationsGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.InvocationsGetResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call invocations.get: " + error.Status.Detail), error.Trailers);
    }
    return response.Invocation ?? throw new InvalidOperationException("Read Invocations.Get: Missing Payload");
  }
  public async Task<global::Harmos.InstanceState> Cancel(ulong instance, string reason, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.InvocationsCancelRequest
    {
      Instance = instance,
      Reason = reason,
    };
    var method = new Method<global::Harmos.Clients.InvocationsCancelRequest, global::Harmos.Clients.InvocationsCancelResponse>(MethodType.Unary, "application.host.v1.Invocations", "Cancel",
        Marshallers.Create<global::Harmos.Clients.InvocationsCancelRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.InvocationsCancelRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.InvocationsCancelResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.InvocationsCancelResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.InvocationsCancelResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call invocations.cancel: " + error.Status.Detail), error.Trailers);
    }
    return response.Invocation ?? throw new InvalidOperationException("Read Invocations.Cancel: Missing Payload");
  }
  public async IAsyncEnumerable<global::Harmos.InstanceState> Watch(ulong instance, CallOptions context = default, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.InvocationsWatchRequest
    {
      Instance = instance,
    };
    var method = new Method<global::Harmos.Clients.InvocationsWatchRequest, global::Harmos.Clients.InvocationsWatchResponse>(MethodType.ServerStreaming, "application.host.v1.Invocations", "Watch",
        Marshallers.Create<global::Harmos.Clients.InvocationsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.InvocationsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.InvocationsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.InvocationsWatchResponse.Parser.ParseFrom(bytes)));
    using var linked = CancellationTokenSource.CreateLinkedTokenSource(context.CancellationToken, cancellationToken);
    using var call = invoker.AsyncServerStreamingCall(method, null, context.WithCancellationToken(linked.Token), request);
    while (true)
    {
      bool available;
      try
      {
        available = await call.ResponseStream.MoveNext(linked.Token);
      }
      catch (RpcException error)
      {
        throw new RpcException(new Status(error.StatusCode, "Call invocations.watch: " + error.Status.Detail), error.Trailers);
      }
      if (!available)
      {
        yield break;
      }
      var response = call.ResponseStream.Current;
      yield return response.Invocation ?? throw new InvalidOperationException("Read Invocations.Watch: Missing Payload");
    }
  }
}
public sealed class JobsResource
{
  private readonly CallInvoker invoker;
  public JobsProgressResource Progress { get; }
  public JobsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
    Progress = new JobsProgressResource(invoker);
  }
  public async Task<string> Start(string value, CallOptions context = default)
  {
    var request = new global::Application.Clients.JobsStartRequest
    {
      Value = value,
    };
    var method = new Method<global::Application.Clients.JobsStartRequest, global::Application.Clients.JobsStartResponse>(MethodType.Unary, "application.host.v1.Jobs", "Start",
        Marshallers.Create<global::Application.Clients.JobsStartRequest>(value => value.ToByteArray(), bytes => global::Application.Clients.JobsStartRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Application.Clients.JobsStartResponse>(value => value.ToByteArray(), bytes => global::Application.Clients.JobsStartResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Application.Clients.JobsStartResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call jobs.start: " + error.Status.Detail), error.Trailers);
    }
    return response.Value;
  }
}
public sealed class JobsProgressResource
{
  private readonly CallInvoker invoker;
  public JobsProgressResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async IAsyncEnumerable<string> Watch(string value, CallOptions context = default, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new global::Application.Clients.ProgressWatchRequest
    {
      Value = value,
    };
    var method = new Method<global::Application.Clients.ProgressWatchRequest, global::Application.Clients.ProgressWatchResponse>(MethodType.ServerStreaming, "application.host.v1.jobs.Progress", "Watch",
        Marshallers.Create<global::Application.Clients.ProgressWatchRequest>(value => value.ToByteArray(), bytes => global::Application.Clients.ProgressWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Application.Clients.ProgressWatchResponse>(value => value.ToByteArray(), bytes => global::Application.Clients.ProgressWatchResponse.Parser.ParseFrom(bytes)));
    using var linked = CancellationTokenSource.CreateLinkedTokenSource(context.CancellationToken, cancellationToken);
    using var call = invoker.AsyncServerStreamingCall(method, null, context.WithCancellationToken(linked.Token), request);
    while (true)
    {
      bool available;
      try
      {
        available = await call.ResponseStream.MoveNext(linked.Token);
      }
      catch (RpcException error)
      {
        throw new RpcException(new Status(error.StatusCode, "Call jobs.progress.watch: " + error.Status.Detail), error.Trailers);
      }
      if (!available)
      {
        yield break;
      }
      var response = call.ResponseStream.Current;
      yield return response.Value;
    }
  }
}
public sealed class OperationsResource
{
  private readonly CallInvoker invoker;
  public OperationsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<IReadOnlyList<global::Harmos.DiscoveredOperation>> List(CallOptions context = default)
  {
    var request = new global::Harmos.Clients.OperationsListRequest
    {
    };
    var method = new Method<global::Harmos.Clients.OperationsListRequest, global::Harmos.Clients.OperationsListResponse>(MethodType.Unary, "application.host.v1.Operations", "List",
        Marshallers.Create<global::Harmos.Clients.OperationsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.OperationsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.OperationsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.OperationsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.OperationsListResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call operations.list: " + error.Status.Detail), error.Trailers);
    }
    return response.Operations;
  }
  public async Task<global::Harmos.DiscoveredOperation> Get(string id, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.OperationsGetRequest
    {
      Id = id,
    };
    var method = new Method<global::Harmos.Clients.OperationsGetRequest, global::Harmos.Clients.OperationsGetResponse>(MethodType.Unary, "application.host.v1.Operations", "Get",
        Marshallers.Create<global::Harmos.Clients.OperationsGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.OperationsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.OperationsGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.OperationsGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.OperationsGetResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call operations.get: " + error.Status.Detail), error.Trailers);
    }
    return response.Operation ?? throw new InvalidOperationException("Read Operations.Get: Missing Payload");
  }
}
public sealed class PluginsResource
{
  private readonly CallInvoker invoker;
  public PluginsAnnotationsResource Annotations { get; }
  public PluginsSettingsResource Settings { get; }
  public PluginsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
    Annotations = new PluginsAnnotationsResource(invoker);
    Settings = new PluginsSettingsResource(invoker);
  }
  public async Task<global::Harmos.Clients.PluginsGetResponse> Get(string name, global::Harmos.ReadCut? at = null, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.PluginsGetRequest
    {
      Name = name,
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<global::Harmos.Clients.PluginsGetRequest, global::Harmos.Clients.PluginsGetResponse>(MethodType.Unary, "application.host.v1.Plugins", "Get",
        Marshallers.Create<global::Harmos.Clients.PluginsGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.PluginsGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.PluginsGetResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call plugins.get: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
  public async Task<global::Harmos.Clients.PluginsListResponse> List(global::Harmos.ReadCut? at = null, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.PluginsListRequest
    {
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<global::Harmos.Clients.PluginsListRequest, global::Harmos.Clients.PluginsListResponse>(MethodType.Unary, "application.host.v1.Plugins", "List",
        Marshallers.Create<global::Harmos.Clients.PluginsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.PluginsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.PluginsListResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call plugins.list: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
  public async IAsyncEnumerable<global::Harmos.Clients.PluginsWatchResponse> Watch(string? name = null, global::Harmos.ReadCut? at = null, CallOptions context = default, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.PluginsWatchRequest
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
    var method = new Method<global::Harmos.Clients.PluginsWatchRequest, global::Harmos.Clients.PluginsWatchResponse>(MethodType.ServerStreaming, "application.host.v1.Plugins", "Watch",
        Marshallers.Create<global::Harmos.Clients.PluginsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.PluginsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsWatchResponse.Parser.ParseFrom(bytes)));
    using var linked = CancellationTokenSource.CreateLinkedTokenSource(context.CancellationToken, cancellationToken);
    using var call = invoker.AsyncServerStreamingCall(method, null, context.WithCancellationToken(linked.Token), request);
    while (true)
    {
      bool available;
      try
      {
        available = await call.ResponseStream.MoveNext(linked.Token);
      }
      catch (RpcException error)
      {
        throw new RpcException(new Status(error.StatusCode, "Call plugins.watch: " + error.Status.Detail), error.Trailers);
      }
      if (!available)
      {
        yield break;
      }
      var response = call.ResponseStream.Current;
      yield return response;
    }
  }
  public async Task<global::Harmos.Clients.PluginsCreateResponse> Create(string name, global::Harmos.Clients.PluginPlacement placement, global::Harmos.Clients.PluginValue value, string? requestId = null, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.PluginsCreateRequest
    {
      RequestId = requestId ?? Guid.NewGuid().ToString("N"),
      Name = name,
      Placement = placement,
      Value = value,
    };
    var method = new Method<global::Harmos.Clients.PluginsCreateRequest, global::Harmos.Clients.PluginsCreateResponse>(MethodType.Unary, "application.host.v1.Plugins", "Create",
        Marshallers.Create<global::Harmos.Clients.PluginsCreateRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.PluginsCreateResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsCreateResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.PluginsCreateResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call plugins.create: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
  public async Task<global::Harmos.Clients.PluginsSetResponse> Set(string name, global::Harmos.Clients.PluginValue value, ByteString expectedRevision, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.PluginsSetRequest
    {
      Name = name,
      Value = value,
      ExpectedRevision = expectedRevision,
    };
    var method = new Method<global::Harmos.Clients.PluginsSetRequest, global::Harmos.Clients.PluginsSetResponse>(MethodType.Unary, "application.host.v1.Plugins", "Set",
        Marshallers.Create<global::Harmos.Clients.PluginsSetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsSetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.PluginsSetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsSetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.PluginsSetResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call plugins.set: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
  public async Task<global::Harmos.Receipt> Delete(string name, ByteString expectedRevision, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.PluginsDeleteRequest
    {
      Name = name,
      ExpectedRevision = expectedRevision,
    };
    var method = new Method<global::Harmos.Clients.PluginsDeleteRequest, global::Harmos.Clients.PluginsDeleteResponse>(MethodType.Unary, "application.host.v1.Plugins", "Delete",
        Marshallers.Create<global::Harmos.Clients.PluginsDeleteRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsDeleteRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.PluginsDeleteResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsDeleteResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.PluginsDeleteResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call plugins.delete: " + error.Status.Detail), error.Trailers);
    }
    return response.Receipt ?? throw new InvalidOperationException("Read Plugins.Delete: Missing Payload");
  }
  public async Task<global::Harmos.Clients.PluginEntry> Restart(string name, ByteString expectedRevision, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.PluginsRestartRequest
    {
      Name = name,
      ExpectedRevision = expectedRevision,
    };
    var method = new Method<global::Harmos.Clients.PluginsRestartRequest, global::Harmos.Clients.PluginsRestartResponse>(MethodType.Unary, "application.host.v1.Plugins", "Restart",
        Marshallers.Create<global::Harmos.Clients.PluginsRestartRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsRestartRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.PluginsRestartResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsRestartResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.PluginsRestartResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call plugins.restart: " + error.Status.Detail), error.Trailers);
    }
    return response.Entry ?? throw new InvalidOperationException("Read Plugins.Restart: Missing Payload");
  }
  public async Task<global::Harmos.Clients.PluginsReplaceResponse> Replace(string name, global::Harmos.Clients.PluginPlacement placement, ByteString expectedRevision, string? requestId = null, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.PluginsReplaceRequest
    {
      Name = name,
      Placement = placement,
      ExpectedRevision = expectedRevision,
      RequestId = requestId ?? Guid.NewGuid().ToString("N"),
    };
    var method = new Method<global::Harmos.Clients.PluginsReplaceRequest, global::Harmos.Clients.PluginsReplaceResponse>(MethodType.Unary, "application.host.v1.Plugins", "Replace",
        Marshallers.Create<global::Harmos.Clients.PluginsReplaceRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsReplaceRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.PluginsReplaceResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.PluginsReplaceResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.PluginsReplaceResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call plugins.replace: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
}
public sealed class PluginsAnnotationsResource
{
  private readonly CallInvoker invoker;
  public PluginsAnnotationsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<string> Get(string name, CallOptions context = default)
  {
    var request = new global::Application.Clients.AnnotationsGetRequest
    {
      Name = name,
    };
    var method = new Method<global::Application.Clients.AnnotationsGetRequest, global::Application.Clients.AnnotationsGetResponse>(MethodType.Unary, "application.host.v1.plugins.Annotations", "Get",
        Marshallers.Create<global::Application.Clients.AnnotationsGetRequest>(value => value.ToByteArray(), bytes => global::Application.Clients.AnnotationsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Application.Clients.AnnotationsGetResponse>(value => value.ToByteArray(), bytes => global::Application.Clients.AnnotationsGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Application.Clients.AnnotationsGetResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call plugins.annotations.get: " + error.Status.Detail), error.Trailers);
    }
    return response.Value;
  }
}
public sealed class PluginsSettingsResource
{
  private readonly CallInvoker invoker;
  public PluginsSettingsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<global::Harmos.Clients.SettingsListResponse> List(string? name = null, global::Harmos.ReadCut? at = null, CallOptions context = default)
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
    var method = new Method<global::Harmos.Clients.SettingsListRequest, global::Harmos.Clients.SettingsListResponse>(MethodType.Unary, "application.host.v1.plugins.Settings", "List",
        Marshallers.Create<global::Harmos.Clients.SettingsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.SettingsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.SettingsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.SettingsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.SettingsListResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call plugins.settings.list: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
  public async Task<global::Harmos.Clients.SettingsSetResponse> Set(string name, global::Harmos.SettingValues value, ByteString expectedRevision, string? requestId = null, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.SettingsSetRequest
    {
      Name = name,
      Value = value,
      ExpectedRevision = expectedRevision,
      RequestId = requestId ?? Guid.NewGuid().ToString("N"),
    };
    var method = new Method<global::Harmos.Clients.SettingsSetRequest, global::Harmos.Clients.SettingsSetResponse>(MethodType.Unary, "application.host.v1.plugins.Settings", "Set",
        Marshallers.Create<global::Harmos.Clients.SettingsSetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.SettingsSetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.SettingsSetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.SettingsSetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.SettingsSetResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call plugins.settings.set: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
}
public sealed class SchemasResource
{
  private readonly CallInvoker invoker;
  public SchemasResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<IReadOnlyList<global::Harmos.Clients.SchemaDescription>> List(string? schema = null, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.SchemasListRequest
    {
    };
    if (schema is not null)
    {
      request.Schema = schema;
    }
    var method = new Method<global::Harmos.Clients.SchemasListRequest, global::Harmos.Clients.SchemasListResponse>(MethodType.Unary, "application.host.v1.Schemas", "List",
        Marshallers.Create<global::Harmos.Clients.SchemasListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.SchemasListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.SchemasListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.SchemasListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.SchemasListResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call schemas.list: " + error.Status.Detail), error.Trailers);
    }
    return response.Schemas;
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
  public async Task<global::Harmos.Clients.StatusGetResponse> Get(CallOptions context = default)
  {
    var request = new global::Harmos.Clients.StatusGetRequest
    {
    };
    var method = new Method<global::Harmos.Clients.StatusGetRequest, global::Harmos.Clients.StatusGetResponse>(MethodType.Unary, "application.host.v1.Status", "Get",
        Marshallers.Create<global::Harmos.Clients.StatusGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.StatusGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.StatusGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.StatusGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.StatusGetResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call status.get: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
  public async IAsyncEnumerable<global::Harmos.Clients.StatusWatchResponse> Watch(global::Harmos.Position? at = null, CallOptions context = default, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.StatusWatchRequest
    {
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<global::Harmos.Clients.StatusWatchRequest, global::Harmos.Clients.StatusWatchResponse>(MethodType.ServerStreaming, "application.host.v1.Status", "Watch",
        Marshallers.Create<global::Harmos.Clients.StatusWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.StatusWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.StatusWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.StatusWatchResponse.Parser.ParseFrom(bytes)));
    using var linked = CancellationTokenSource.CreateLinkedTokenSource(context.CancellationToken, cancellationToken);
    using var call = invoker.AsyncServerStreamingCall(method, null, context.WithCancellationToken(linked.Token), request);
    while (true)
    {
      bool available;
      try
      {
        available = await call.ResponseStream.MoveNext(linked.Token);
      }
      catch (RpcException error)
      {
        throw new RpcException(new Status(error.StatusCode, "Call status.watch: " + error.Status.Detail), error.Trailers);
      }
      if (!available)
      {
        yield break;
      }
      var response = call.ResponseStream.Current;
      yield return response;
    }
  }
}
public sealed class StatusDiagnosticsResource
{
  private readonly CallInvoker invoker;
  public StatusDiagnosticsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<global::Harmos.Clients.DiagnosticsGetResponse> Get(string name, ulong offset, ulong maxBytes, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.DiagnosticsGetRequest
    {
      Name = name,
      Offset = offset,
      MaxBytes = maxBytes,
    };
    var method = new Method<global::Harmos.Clients.DiagnosticsGetRequest, global::Harmos.Clients.DiagnosticsGetResponse>(MethodType.Unary, "application.host.v1.status.Diagnostics", "Get",
        Marshallers.Create<global::Harmos.Clients.DiagnosticsGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.DiagnosticsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.DiagnosticsGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.DiagnosticsGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.DiagnosticsGetResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call status.diagnostics.get: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
}
public sealed class StatusLogsResource
{
  private readonly CallInvoker invoker;
  public StatusLogsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<global::Harmos.Clients.LogsListResponse> List(global::Harmos.Position? at = null, global::Harmos.PageRequest? page = null, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.LogsListRequest
    {
    };
    if (at is not null)
    {
      request.At = at;
    }
    if (page is not null)
    {
      request.Page = page;
    }
    var method = new Method<global::Harmos.Clients.LogsListRequest, global::Harmos.Clients.LogsListResponse>(MethodType.Unary, "application.host.v1.status.Logs", "List",
        Marshallers.Create<global::Harmos.Clients.LogsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.LogsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.LogsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.LogsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.LogsListResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call status.logs.list: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
  public async IAsyncEnumerable<global::Harmos.Clients.LogsWatchResponse> Watch(ulong maxBytes, uint maxEvents, global::Harmos.Position? at = null, CallOptions context = default, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.LogsWatchRequest
    {
      MaxBytes = maxBytes,
      MaxEvents = maxEvents,
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<global::Harmos.Clients.LogsWatchRequest, global::Harmos.Clients.LogsWatchResponse>(MethodType.ServerStreaming, "application.host.v1.status.Logs", "Watch",
        Marshallers.Create<global::Harmos.Clients.LogsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.LogsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.LogsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.LogsWatchResponse.Parser.ParseFrom(bytes)));
    using var linked = CancellationTokenSource.CreateLinkedTokenSource(context.CancellationToken, cancellationToken);
    using var call = invoker.AsyncServerStreamingCall(method, null, context.WithCancellationToken(linked.Token), request);
    while (true)
    {
      bool available;
      try
      {
        available = await call.ResponseStream.MoveNext(linked.Token);
      }
      catch (RpcException error)
      {
        throw new RpcException(new Status(error.StatusCode, "Call status.logs.watch: " + error.Status.Detail), error.Trailers);
      }
      if (!available)
      {
        yield break;
      }
      var response = call.ResponseStream.Current;
      yield return response;
    }
  }
}
public sealed class StatusTraceEventsResource
{
  private readonly CallInvoker invoker;
  public StatusTraceEventsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<global::Harmos.Clients.TraceEventsListResponse> List(global::Harmos.Position? at = null, global::Harmos.PageRequest? page = null, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.TraceEventsListRequest
    {
    };
    if (at is not null)
    {
      request.At = at;
    }
    if (page is not null)
    {
      request.Page = page;
    }
    var method = new Method<global::Harmos.Clients.TraceEventsListRequest, global::Harmos.Clients.TraceEventsListResponse>(MethodType.Unary, "application.host.v1.status.TraceEvents", "List",
        Marshallers.Create<global::Harmos.Clients.TraceEventsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.TraceEventsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.TraceEventsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.TraceEventsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.TraceEventsListResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call status.trace_events.list: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
  public async IAsyncEnumerable<global::Harmos.Clients.TraceEventsWatchResponse> Watch(ulong maxBytes, uint maxEvents, global::Harmos.Position? at = null, CallOptions context = default, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new global::Harmos.Clients.TraceEventsWatchRequest
    {
      MaxBytes = maxBytes,
      MaxEvents = maxEvents,
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<global::Harmos.Clients.TraceEventsWatchRequest, global::Harmos.Clients.TraceEventsWatchResponse>(MethodType.ServerStreaming, "application.host.v1.status.TraceEvents", "Watch",
        Marshallers.Create<global::Harmos.Clients.TraceEventsWatchRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.TraceEventsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.TraceEventsWatchResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.TraceEventsWatchResponse.Parser.ParseFrom(bytes)));
    using var linked = CancellationTokenSource.CreateLinkedTokenSource(context.CancellationToken, cancellationToken);
    using var call = invoker.AsyncServerStreamingCall(method, null, context.WithCancellationToken(linked.Token), request);
    while (true)
    {
      bool available;
      try
      {
        available = await call.ResponseStream.MoveNext(linked.Token);
      }
      catch (RpcException error)
      {
        throw new RpcException(new Status(error.StatusCode, "Call status.trace_events.watch: " + error.Status.Detail), error.Trailers);
      }
      if (!available)
      {
        yield break;
      }
      var response = call.ResponseStream.Current;
      yield return response;
    }
  }
}
public sealed class StreamsResource
{
  private readonly CallInvoker invoker;
  public StreamsResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<IReadOnlyList<global::Harmos.Clients.StreamIdentity>> List(string? schema = null, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.StreamsListRequest
    {
    };
    if (schema is not null)
    {
      request.Schema = schema;
    }
    var method = new Method<global::Harmos.Clients.StreamsListRequest, global::Harmos.Clients.StreamsListResponse>(MethodType.Unary, "application.host.v1.Streams", "List",
        Marshallers.Create<global::Harmos.Clients.StreamsListRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.StreamsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.StreamsListResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.StreamsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.StreamsListResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call streams.list: " + error.Status.Detail), error.Trailers);
    }
    return response.Streams;
  }
}
public sealed class TimeResource
{
  private readonly CallInvoker invoker;
  public TimeResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public async Task<global::Google.Protobuf.WellKnownTypes.Timestamp> Get(CallOptions context = default)
  {
    var request = new global::Harmos.Clients.TimeGetRequest
    {
    };
    var method = new Method<global::Harmos.Clients.TimeGetRequest, global::Harmos.Clients.TimeGetResponse>(MethodType.Unary, "application.host.v1.Time", "Get",
        Marshallers.Create<global::Harmos.Clients.TimeGetRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.TimeGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.TimeGetResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.TimeGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.TimeGetResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call time.get: " + error.Status.Detail), error.Trailers);
    }
    return response.Now ?? throw new InvalidOperationException("Read Time.Get: Missing Payload");
  }
  public async Task<global::Harmos.Clients.TimeSleepResponse> Sleep(global::Google.Protobuf.WellKnownTypes.Duration duration, CallOptions context = default)
  {
    var request = new global::Harmos.Clients.TimeSleepRequest
    {
      Duration = duration,
    };
    var method = new Method<global::Harmos.Clients.TimeSleepRequest, global::Harmos.Clients.TimeSleepResponse>(MethodType.Unary, "application.host.v1.Time", "Sleep",
        Marshallers.Create<global::Harmos.Clients.TimeSleepRequest>(value => value.ToByteArray(), bytes => global::Harmos.Clients.TimeSleepRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Harmos.Clients.TimeSleepResponse>(value => value.ToByteArray(), bytes => global::Harmos.Clients.TimeSleepResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    global::Harmos.Clients.TimeSleepResponse response;
    try
    {
      response = await call.ResponseAsync;
    }
    catch (RpcException error)
    {
      throw new RpcException(new Status(error.StatusCode, "Call time.sleep: " + error.Status.Detail), error.Trailers);
    }
    return response;
  }
}
