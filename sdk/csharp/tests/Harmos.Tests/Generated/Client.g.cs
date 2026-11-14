// @generated from protocol descriptors; do not edit.
#nullable enable
using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using System.Runtime.CompilerServices;
using Grpc.Core;
using Grpc.Net.Client;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using StorageTransferSettings = global::Harmos.StorageTransferSettings;
using Transfer = global::Harmos.Transfer;
using AnnotationsGetRequest = global::Application.Clients.AnnotationsGetRequest;
using AnnotationsGetResponse = global::Application.Clients.AnnotationsGetResponse;
using JobsStartRequest = global::Application.Clients.JobsStartRequest;
using JobsStartResponse = global::Application.Clients.JobsStartResponse;
using ProgressWatchRequest = global::Application.Clients.ProgressWatchRequest;
using ProgressWatchResponse = global::Application.Clients.ProgressWatchResponse;
using Duration = global::Google.Protobuf.WellKnownTypes.Duration;
using CertificatesCreateRequest = global::Harmos.Clients.CertificatesCreateRequest;
using CertificatesCreateResponse = global::Harmos.Clients.CertificatesCreateResponse;
using ListenerGetRequest = global::Harmos.Clients.ListenerGetRequest;
using ListenerGetResponse = global::Harmos.Clients.ListenerGetResponse;
using ListenerSetRequest = global::Harmos.Clients.ListenerSetRequest;
using ListenerSetResponse = global::Harmos.Clients.ListenerSetResponse;
using ListenerValue = global::Harmos.Clients.ListenerValue;
using PairingsCreateRequest = global::Harmos.Clients.PairingsCreateRequest;
using PairingsCreateResponse = global::Harmos.Clients.PairingsCreateResponse;
using PairingsDeleteRequest = global::Harmos.Clients.PairingsDeleteRequest;
using PairingsDeleteResponse = global::Harmos.Clients.PairingsDeleteResponse;
using PairingsListRequest = global::Harmos.Clients.PairingsListRequest;
using PairingsListResponse = global::Harmos.Clients.PairingsListResponse;
using InvocationsCancelRequest = global::Harmos.Clients.InvocationsCancelRequest;
using InvocationsCancelResponse = global::Harmos.Clients.InvocationsCancelResponse;
using InvocationsCreateRequest = global::Harmos.Clients.InvocationsCreateRequest;
using InvocationsCreateResponse = global::Harmos.Clients.InvocationsCreateResponse;
using InvocationsGetRequest = global::Harmos.Clients.InvocationsGetRequest;
using InvocationsGetResponse = global::Harmos.Clients.InvocationsGetResponse;
using InvocationsWatchRequest = global::Harmos.Clients.InvocationsWatchRequest;
using InvocationsWatchResponse = global::Harmos.Clients.InvocationsWatchResponse;
using OperationsGetRequest = global::Harmos.Clients.OperationsGetRequest;
using OperationsGetResponse = global::Harmos.Clients.OperationsGetResponse;
using OperationsListRequest = global::Harmos.Clients.OperationsListRequest;
using OperationsListResponse = global::Harmos.Clients.OperationsListResponse;
using PluginEntry = global::Harmos.Clients.PluginEntry;
using PluginPlacement = global::Harmos.Clients.PluginPlacement;
using PluginValue = global::Harmos.Clients.PluginValue;
using PluginsCreateRequest = global::Harmos.Clients.PluginsCreateRequest;
using PluginsCreateResponse = global::Harmos.Clients.PluginsCreateResponse;
using PluginsDeleteRequest = global::Harmos.Clients.PluginsDeleteRequest;
using PluginsDeleteResponse = global::Harmos.Clients.PluginsDeleteResponse;
using PluginsGetRequest = global::Harmos.Clients.PluginsGetRequest;
using PluginsGetResponse = global::Harmos.Clients.PluginsGetResponse;
using PluginsListRequest = global::Harmos.Clients.PluginsListRequest;
using PluginsListResponse = global::Harmos.Clients.PluginsListResponse;
using PluginsReplaceRequest = global::Harmos.Clients.PluginsReplaceRequest;
using PluginsReplaceResponse = global::Harmos.Clients.PluginsReplaceResponse;
using SettingsListRequest = global::Harmos.Clients.SettingsListRequest;
using SettingsListResponse = global::Harmos.Clients.SettingsListResponse;
using SettingsSetRequest = global::Harmos.Clients.SettingsSetRequest;
using SettingsSetResponse = global::Harmos.Clients.SettingsSetResponse;
using PluginsRestartRequest = global::Harmos.Clients.PluginsRestartRequest;
using PluginsRestartResponse = global::Harmos.Clients.PluginsRestartResponse;
using PluginsSetRequest = global::Harmos.Clients.PluginsSetRequest;
using PluginsSetResponse = global::Harmos.Clients.PluginsSetResponse;
using PluginsWatchRequest = global::Harmos.Clients.PluginsWatchRequest;
using PluginsWatchResponse = global::Harmos.Clients.PluginsWatchResponse;
using SchemaDescription = global::Harmos.Clients.SchemaDescription;
using SchemasListRequest = global::Harmos.Clients.SchemasListRequest;
using SchemasListResponse = global::Harmos.Clients.SchemasListResponse;
using StatusGetRequest = global::Harmos.Clients.StatusGetRequest;
using StatusGetResponse = global::Harmos.Clients.StatusGetResponse;
using DiagnosticsGetRequest = global::Harmos.Clients.DiagnosticsGetRequest;
using DiagnosticsGetResponse = global::Harmos.Clients.DiagnosticsGetResponse;
using LogsListRequest = global::Harmos.Clients.LogsListRequest;
using LogsListResponse = global::Harmos.Clients.LogsListResponse;
using LogsWatchRequest = global::Harmos.Clients.LogsWatchRequest;
using LogsWatchResponse = global::Harmos.Clients.LogsWatchResponse;
using TraceEventsListRequest = global::Harmos.Clients.TraceEventsListRequest;
using TraceEventsListResponse = global::Harmos.Clients.TraceEventsListResponse;
using TraceEventsWatchRequest = global::Harmos.Clients.TraceEventsWatchRequest;
using TraceEventsWatchResponse = global::Harmos.Clients.TraceEventsWatchResponse;
using StatusWatchRequest = global::Harmos.Clients.StatusWatchRequest;
using StatusWatchResponse = global::Harmos.Clients.StatusWatchResponse;
using StreamIdentity = global::Harmos.Clients.StreamIdentity;
using StreamsListRequest = global::Harmos.Clients.StreamsListRequest;
using StreamsListResponse = global::Harmos.Clients.StreamsListResponse;
using TimeSleepRequest = global::Harmos.Clients.TimeSleepRequest;
using TimeSleepResponse = global::Harmos.Clients.TimeSleepResponse;
using DiscoveredOperation = global::Harmos.DiscoveredOperation;
using InstanceState = global::Harmos.InstanceState;
using PageRequest = global::Harmos.PageRequest;
using Position = global::Harmos.Position;
using ReadCut = global::Harmos.ReadCut;
using Receipt = global::Harmos.Receipt;
using SettingValues = global::Harmos.SettingValues;
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
  public static Client Connect(string address, StorageTransferSettings? transfer = null, int? messageBytes = null)
  {
    var maximum = Transfer.MessageBytes(transfer, messageBytes);
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
    var request = new CertificatesCreateRequest
    {
    };
    var method = new Method<CertificatesCreateRequest, CertificatesCreateResponse>(MethodType.Unary, "harmos.host.v1.access.Certificates", "Create",
        Marshallers.Create<CertificatesCreateRequest>(value => value.ToByteArray(), bytes => CertificatesCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<CertificatesCreateResponse>(value => value.ToByteArray(), bytes => CertificatesCreateResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    CertificatesCreateResponse response;
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
  public async Task<ListenerGetResponse> Get(CallOptions context = default)
  {
    var request = new ListenerGetRequest
    {
    };
    var method = new Method<ListenerGetRequest, ListenerGetResponse>(MethodType.Unary, "harmos.host.v1.access.Listener", "Get",
        Marshallers.Create<ListenerGetRequest>(value => value.ToByteArray(), bytes => ListenerGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<ListenerGetResponse>(value => value.ToByteArray(), bytes => ListenerGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    ListenerGetResponse response;
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
  public async Task<ListenerSetResponse> Set(ListenerValue value, ByteString expectedRevision, CallOptions context = default)
  {
    var request = new ListenerSetRequest
    {
      Value = value,
      ExpectedRevision = expectedRevision,
    };
    var method = new Method<ListenerSetRequest, ListenerSetResponse>(MethodType.Unary, "harmos.host.v1.access.Listener", "Set",
        Marshallers.Create<ListenerSetRequest>(value => value.ToByteArray(), bytes => ListenerSetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<ListenerSetResponse>(value => value.ToByteArray(), bytes => ListenerSetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    ListenerSetResponse response;
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
  public async Task<PairingsCreateResponse> Create(string label, CallOptions context = default)
  {
    var request = new PairingsCreateRequest
    {
      Label = label,
    };
    var method = new Method<PairingsCreateRequest, PairingsCreateResponse>(MethodType.Unary, "harmos.host.v1.access.Pairings", "Create",
        Marshallers.Create<PairingsCreateRequest>(value => value.ToByteArray(), bytes => PairingsCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<PairingsCreateResponse>(value => value.ToByteArray(), bytes => PairingsCreateResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    PairingsCreateResponse response;
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
  public async Task<PairingsListResponse> List(Position? at = null, CallOptions context = default)
  {
    var request = new PairingsListRequest
    {
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<PairingsListRequest, PairingsListResponse>(MethodType.Unary, "harmos.host.v1.access.Pairings", "List",
        Marshallers.Create<PairingsListRequest>(value => value.ToByteArray(), bytes => PairingsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<PairingsListResponse>(value => value.ToByteArray(), bytes => PairingsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    PairingsListResponse response;
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
  public async Task<PairingsDeleteResponse> Delete(string name, CallOptions context = default)
  {
    var request = new PairingsDeleteRequest
    {
      Name = name,
    };
    var method = new Method<PairingsDeleteRequest, PairingsDeleteResponse>(MethodType.Unary, "harmos.host.v1.access.Pairings", "Delete",
        Marshallers.Create<PairingsDeleteRequest>(value => value.ToByteArray(), bytes => PairingsDeleteRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<PairingsDeleteResponse>(value => value.ToByteArray(), bytes => PairingsDeleteResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    PairingsDeleteResponse response;
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
    var request = new InvocationsCreateRequest
    {
      Operation = operation,
      Fingerprint = fingerprint,
      Payload = payload,
    };
    var method = new Method<InvocationsCreateRequest, InvocationsCreateResponse>(MethodType.Unary, "harmos.host.v1.Invocations", "Create",
        Marshallers.Create<InvocationsCreateRequest>(value => value.ToByteArray(), bytes => InvocationsCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<InvocationsCreateResponse>(value => value.ToByteArray(), bytes => InvocationsCreateResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    InvocationsCreateResponse response;
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
  public async Task<InstanceState> Get(ulong instance, CallOptions context = default)
  {
    var request = new InvocationsGetRequest
    {
      Instance = instance,
    };
    var method = new Method<InvocationsGetRequest, InvocationsGetResponse>(MethodType.Unary, "harmos.host.v1.Invocations", "Get",
        Marshallers.Create<InvocationsGetRequest>(value => value.ToByteArray(), bytes => InvocationsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<InvocationsGetResponse>(value => value.ToByteArray(), bytes => InvocationsGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    InvocationsGetResponse response;
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
  public async Task<InstanceState> Cancel(ulong instance, string reason, CallOptions context = default)
  {
    var request = new InvocationsCancelRequest
    {
      Instance = instance,
      Reason = reason,
    };
    var method = new Method<InvocationsCancelRequest, InvocationsCancelResponse>(MethodType.Unary, "harmos.host.v1.Invocations", "Cancel",
        Marshallers.Create<InvocationsCancelRequest>(value => value.ToByteArray(), bytes => InvocationsCancelRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<InvocationsCancelResponse>(value => value.ToByteArray(), bytes => InvocationsCancelResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    InvocationsCancelResponse response;
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
  public async IAsyncEnumerable<InstanceState> Watch(ulong instance, CallOptions context = default, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new InvocationsWatchRequest
    {
      Instance = instance,
    };
    var method = new Method<InvocationsWatchRequest, InvocationsWatchResponse>(MethodType.ServerStreaming, "harmos.host.v1.Invocations", "Watch",
        Marshallers.Create<InvocationsWatchRequest>(value => value.ToByteArray(), bytes => InvocationsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<InvocationsWatchResponse>(value => value.ToByteArray(), bytes => InvocationsWatchResponse.Parser.ParseFrom(bytes)));
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
    var request = new JobsStartRequest
    {
      Value = value,
    };
    var method = new Method<JobsStartRequest, JobsStartResponse>(MethodType.Unary, "application.host.v1.Jobs", "Start",
        Marshallers.Create<JobsStartRequest>(value => value.ToByteArray(), bytes => JobsStartRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<JobsStartResponse>(value => value.ToByteArray(), bytes => JobsStartResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    JobsStartResponse response;
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
    var request = new ProgressWatchRequest
    {
      Value = value,
    };
    var method = new Method<ProgressWatchRequest, ProgressWatchResponse>(MethodType.ServerStreaming, "application.host.v1.jobs.Progress", "Watch",
        Marshallers.Create<ProgressWatchRequest>(value => value.ToByteArray(), bytes => ProgressWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<ProgressWatchResponse>(value => value.ToByteArray(), bytes => ProgressWatchResponse.Parser.ParseFrom(bytes)));
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
  public async Task<IReadOnlyList<DiscoveredOperation>> List(CallOptions context = default)
  {
    var request = new OperationsListRequest
    {
    };
    var method = new Method<OperationsListRequest, OperationsListResponse>(MethodType.Unary, "harmos.host.v1.Operations", "List",
        Marshallers.Create<OperationsListRequest>(value => value.ToByteArray(), bytes => OperationsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<OperationsListResponse>(value => value.ToByteArray(), bytes => OperationsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    OperationsListResponse response;
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
  public async Task<DiscoveredOperation> Get(string id, CallOptions context = default)
  {
    var request = new OperationsGetRequest
    {
      Id = id,
    };
    var method = new Method<OperationsGetRequest, OperationsGetResponse>(MethodType.Unary, "harmos.host.v1.Operations", "Get",
        Marshallers.Create<OperationsGetRequest>(value => value.ToByteArray(), bytes => OperationsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<OperationsGetResponse>(value => value.ToByteArray(), bytes => OperationsGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    OperationsGetResponse response;
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
  public async Task<PluginsGetResponse> Get(string name, ReadCut? at = null, CallOptions context = default)
  {
    var request = new PluginsGetRequest
    {
      Name = name,
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<PluginsGetRequest, PluginsGetResponse>(MethodType.Unary, "harmos.host.v1.Plugins", "Get",
        Marshallers.Create<PluginsGetRequest>(value => value.ToByteArray(), bytes => PluginsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<PluginsGetResponse>(value => value.ToByteArray(), bytes => PluginsGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    PluginsGetResponse response;
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
  public async Task<PluginsListResponse> List(ReadCut? at = null, CallOptions context = default)
  {
    var request = new PluginsListRequest
    {
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<PluginsListRequest, PluginsListResponse>(MethodType.Unary, "harmos.host.v1.Plugins", "List",
        Marshallers.Create<PluginsListRequest>(value => value.ToByteArray(), bytes => PluginsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<PluginsListResponse>(value => value.ToByteArray(), bytes => PluginsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    PluginsListResponse response;
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
  public async IAsyncEnumerable<PluginsWatchResponse> Watch(string? name = null, ReadCut? at = null, CallOptions context = default, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new PluginsWatchRequest
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
    var method = new Method<PluginsWatchRequest, PluginsWatchResponse>(MethodType.ServerStreaming, "harmos.host.v1.Plugins", "Watch",
        Marshallers.Create<PluginsWatchRequest>(value => value.ToByteArray(), bytes => PluginsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<PluginsWatchResponse>(value => value.ToByteArray(), bytes => PluginsWatchResponse.Parser.ParseFrom(bytes)));
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
  public async Task<PluginsCreateResponse> Create(string name, PluginPlacement placement, PluginValue value, string? requestId = null, CallOptions context = default)
  {
    var request = new PluginsCreateRequest
    {
      RequestId = requestId ?? Guid.NewGuid().ToString("N"),
      Name = name,
      Placement = placement,
      Value = value,
    };
    var method = new Method<PluginsCreateRequest, PluginsCreateResponse>(MethodType.Unary, "harmos.host.v1.Plugins", "Create",
        Marshallers.Create<PluginsCreateRequest>(value => value.ToByteArray(), bytes => PluginsCreateRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<PluginsCreateResponse>(value => value.ToByteArray(), bytes => PluginsCreateResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    PluginsCreateResponse response;
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
  public async Task<PluginsSetResponse> Set(string name, PluginValue value, ByteString expectedRevision, CallOptions context = default)
  {
    var request = new PluginsSetRequest
    {
      Name = name,
      Value = value,
      ExpectedRevision = expectedRevision,
    };
    var method = new Method<PluginsSetRequest, PluginsSetResponse>(MethodType.Unary, "harmos.host.v1.Plugins", "Set",
        Marshallers.Create<PluginsSetRequest>(value => value.ToByteArray(), bytes => PluginsSetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<PluginsSetResponse>(value => value.ToByteArray(), bytes => PluginsSetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    PluginsSetResponse response;
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
  public async Task<Receipt> Delete(string name, ByteString expectedRevision, CallOptions context = default)
  {
    var request = new PluginsDeleteRequest
    {
      Name = name,
      ExpectedRevision = expectedRevision,
    };
    var method = new Method<PluginsDeleteRequest, PluginsDeleteResponse>(MethodType.Unary, "harmos.host.v1.Plugins", "Delete",
        Marshallers.Create<PluginsDeleteRequest>(value => value.ToByteArray(), bytes => PluginsDeleteRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<PluginsDeleteResponse>(value => value.ToByteArray(), bytes => PluginsDeleteResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    PluginsDeleteResponse response;
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
  public async Task<PluginEntry> Restart(string name, ByteString expectedRevision, CallOptions context = default)
  {
    var request = new PluginsRestartRequest
    {
      Name = name,
      ExpectedRevision = expectedRevision,
    };
    var method = new Method<PluginsRestartRequest, PluginsRestartResponse>(MethodType.Unary, "harmos.host.v1.Plugins", "Restart",
        Marshallers.Create<PluginsRestartRequest>(value => value.ToByteArray(), bytes => PluginsRestartRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<PluginsRestartResponse>(value => value.ToByteArray(), bytes => PluginsRestartResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    PluginsRestartResponse response;
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
  public async Task<PluginsReplaceResponse> Replace(string name, PluginPlacement placement, ByteString expectedRevision, string? requestId = null, CallOptions context = default)
  {
    var request = new PluginsReplaceRequest
    {
      Name = name,
      Placement = placement,
      ExpectedRevision = expectedRevision,
      RequestId = requestId ?? Guid.NewGuid().ToString("N"),
    };
    var method = new Method<PluginsReplaceRequest, PluginsReplaceResponse>(MethodType.Unary, "harmos.host.v1.Plugins", "Replace",
        Marshallers.Create<PluginsReplaceRequest>(value => value.ToByteArray(), bytes => PluginsReplaceRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<PluginsReplaceResponse>(value => value.ToByteArray(), bytes => PluginsReplaceResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    PluginsReplaceResponse response;
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
    var request = new AnnotationsGetRequest
    {
      Name = name,
    };
    var method = new Method<AnnotationsGetRequest, AnnotationsGetResponse>(MethodType.Unary, "application.host.v1.plugins.Annotations", "Get",
        Marshallers.Create<AnnotationsGetRequest>(value => value.ToByteArray(), bytes => AnnotationsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<AnnotationsGetResponse>(value => value.ToByteArray(), bytes => AnnotationsGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    AnnotationsGetResponse response;
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
  public async Task<SettingsListResponse> List(string? name = null, ReadCut? at = null, CallOptions context = default)
  {
    var request = new SettingsListRequest
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
    var method = new Method<SettingsListRequest, SettingsListResponse>(MethodType.Unary, "harmos.host.v1.plugins.Settings", "List",
        Marshallers.Create<SettingsListRequest>(value => value.ToByteArray(), bytes => SettingsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<SettingsListResponse>(value => value.ToByteArray(), bytes => SettingsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    SettingsListResponse response;
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
  public async Task<SettingsSetResponse> Set(string name, SettingValues value, ByteString expectedRevision, string? requestId = null, CallOptions context = default)
  {
    var request = new SettingsSetRequest
    {
      Name = name,
      Value = value,
      ExpectedRevision = expectedRevision,
      RequestId = requestId ?? Guid.NewGuid().ToString("N"),
    };
    var method = new Method<SettingsSetRequest, SettingsSetResponse>(MethodType.Unary, "harmos.host.v1.plugins.Settings", "Set",
        Marshallers.Create<SettingsSetRequest>(value => value.ToByteArray(), bytes => SettingsSetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<SettingsSetResponse>(value => value.ToByteArray(), bytes => SettingsSetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    SettingsSetResponse response;
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
  public async Task<IReadOnlyList<SchemaDescription>> List(string? schema = null, CallOptions context = default)
  {
    var request = new SchemasListRequest
    {
    };
    if (schema is not null)
    {
      request.Schema = schema;
    }
    var method = new Method<SchemasListRequest, SchemasListResponse>(MethodType.Unary, "harmos.host.v1.Schemas", "List",
        Marshallers.Create<SchemasListRequest>(value => value.ToByteArray(), bytes => SchemasListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<SchemasListResponse>(value => value.ToByteArray(), bytes => SchemasListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    SchemasListResponse response;
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
  public async Task<StatusGetResponse> Get(CallOptions context = default)
  {
    var request = new StatusGetRequest
    {
    };
    var method = new Method<StatusGetRequest, StatusGetResponse>(MethodType.Unary, "harmos.host.v1.Status", "Get",
        Marshallers.Create<StatusGetRequest>(value => value.ToByteArray(), bytes => StatusGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<StatusGetResponse>(value => value.ToByteArray(), bytes => StatusGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    StatusGetResponse response;
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
  public async IAsyncEnumerable<StatusWatchResponse> Watch(Position? at = null, CallOptions context = default, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new StatusWatchRequest
    {
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<StatusWatchRequest, StatusWatchResponse>(MethodType.ServerStreaming, "harmos.host.v1.Status", "Watch",
        Marshallers.Create<StatusWatchRequest>(value => value.ToByteArray(), bytes => StatusWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<StatusWatchResponse>(value => value.ToByteArray(), bytes => StatusWatchResponse.Parser.ParseFrom(bytes)));
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
  public async Task<DiagnosticsGetResponse> Get(string name, ulong offset, ulong maxBytes, CallOptions context = default)
  {
    var request = new DiagnosticsGetRequest
    {
      Name = name,
      Offset = offset,
      MaxBytes = maxBytes,
    };
    var method = new Method<DiagnosticsGetRequest, DiagnosticsGetResponse>(MethodType.Unary, "harmos.host.v1.status.Diagnostics", "Get",
        Marshallers.Create<DiagnosticsGetRequest>(value => value.ToByteArray(), bytes => DiagnosticsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<DiagnosticsGetResponse>(value => value.ToByteArray(), bytes => DiagnosticsGetResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    DiagnosticsGetResponse response;
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
  public async Task<LogsListResponse> List(Position? at = null, PageRequest? page = null, CallOptions context = default)
  {
    var request = new LogsListRequest
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
    var method = new Method<LogsListRequest, LogsListResponse>(MethodType.Unary, "harmos.host.v1.status.Logs", "List",
        Marshallers.Create<LogsListRequest>(value => value.ToByteArray(), bytes => LogsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<LogsListResponse>(value => value.ToByteArray(), bytes => LogsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    LogsListResponse response;
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
  public async IAsyncEnumerable<LogsWatchResponse> Watch(ulong maxBytes, uint maxEvents, Position? at = null, CallOptions context = default, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new LogsWatchRequest
    {
      MaxBytes = maxBytes,
      MaxEvents = maxEvents,
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<LogsWatchRequest, LogsWatchResponse>(MethodType.ServerStreaming, "harmos.host.v1.status.Logs", "Watch",
        Marshallers.Create<LogsWatchRequest>(value => value.ToByteArray(), bytes => LogsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<LogsWatchResponse>(value => value.ToByteArray(), bytes => LogsWatchResponse.Parser.ParseFrom(bytes)));
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
  public async Task<TraceEventsListResponse> List(Position? at = null, PageRequest? page = null, CallOptions context = default)
  {
    var request = new TraceEventsListRequest
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
    var method = new Method<TraceEventsListRequest, TraceEventsListResponse>(MethodType.Unary, "harmos.host.v1.status.TraceEvents", "List",
        Marshallers.Create<TraceEventsListRequest>(value => value.ToByteArray(), bytes => TraceEventsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<TraceEventsListResponse>(value => value.ToByteArray(), bytes => TraceEventsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    TraceEventsListResponse response;
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
  public async IAsyncEnumerable<TraceEventsWatchResponse> Watch(ulong maxBytes, uint maxEvents, Position? at = null, CallOptions context = default, [EnumeratorCancellation] CancellationToken cancellationToken = default)
  {
    var request = new TraceEventsWatchRequest
    {
      MaxBytes = maxBytes,
      MaxEvents = maxEvents,
    };
    if (at is not null)
    {
      request.At = at;
    }
    var method = new Method<TraceEventsWatchRequest, TraceEventsWatchResponse>(MethodType.ServerStreaming, "harmos.host.v1.status.TraceEvents", "Watch",
        Marshallers.Create<TraceEventsWatchRequest>(value => value.ToByteArray(), bytes => TraceEventsWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<TraceEventsWatchResponse>(value => value.ToByteArray(), bytes => TraceEventsWatchResponse.Parser.ParseFrom(bytes)));
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
  public async Task<IReadOnlyList<StreamIdentity>> List(string? schema = null, CallOptions context = default)
  {
    var request = new StreamsListRequest
    {
    };
    if (schema is not null)
    {
      request.Schema = schema;
    }
    var method = new Method<StreamsListRequest, StreamsListResponse>(MethodType.Unary, "harmos.host.v1.Streams", "List",
        Marshallers.Create<StreamsListRequest>(value => value.ToByteArray(), bytes => StreamsListRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<StreamsListResponse>(value => value.ToByteArray(), bytes => StreamsListResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    StreamsListResponse response;
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
  public async Task<TimeSleepResponse> Sleep(Duration duration, CallOptions context = default)
  {
    var request = new TimeSleepRequest
    {
      Duration = duration,
    };
    var method = new Method<TimeSleepRequest, TimeSleepResponse>(MethodType.Unary, "harmos.host.v1.Time", "Sleep",
        Marshallers.Create<TimeSleepRequest>(value => value.ToByteArray(), bytes => TimeSleepRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<TimeSleepResponse>(value => value.ToByteArray(), bytes => TimeSleepResponse.Parser.ParseFrom(bytes)));
    using var call = invoker.AsyncUnaryCall(method, null, context, request);
    TimeSleepResponse response;
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
