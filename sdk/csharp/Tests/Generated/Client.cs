// @generated from the protocol descriptor table; do not edit.
using Grpc.Core;
using Google.Protobuf;
namespace Fixture.Sdk;
public sealed class Client
{
  public JobsResource Jobs { get; }
  public PluginsResource Plugins { get; }
  public Client(CallInvoker invoker)
  {
    Jobs = new JobsResource(invoker);
    Plugins = new PluginsResource(invoker);
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
  public AsyncUnaryCall<global::Application.V1.JobsStartResponse> Start(global::Application.V1.JobsStartRequest request, CallOptions options = default)
  {
    var method = new Method<global::Application.V1.JobsStartRequest, global::Application.V1.JobsStartResponse>(MethodType.Unary, "application.v1.Jobs", "Start",
        Marshallers.Create<global::Application.V1.JobsStartRequest>(value => value.ToByteArray(), bytes => global::Application.V1.JobsStartRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Application.V1.JobsStartResponse>(value => value.ToByteArray(), bytes => global::Application.V1.JobsStartResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
public sealed class JobsProgressResource
{
  private readonly CallInvoker invoker;
  public JobsProgressResource(CallInvoker invoker)
  {
    this.invoker = invoker;
  }
  public AsyncServerStreamingCall<global::Application.V1.Jobs.ProgressWatchResponse> Watch(global::Application.V1.Jobs.ProgressWatchRequest request, CallOptions options = default)
  {
    var method = new Method<global::Application.V1.Jobs.ProgressWatchRequest, global::Application.V1.Jobs.ProgressWatchResponse>(MethodType.ServerStreaming, "application.v1.jobs.Progress", "Watch",
        Marshallers.Create<global::Application.V1.Jobs.ProgressWatchRequest>(value => value.ToByteArray(), bytes => global::Application.V1.Jobs.ProgressWatchRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Application.V1.Jobs.ProgressWatchResponse>(value => value.ToByteArray(), bytes => global::Application.V1.Jobs.ProgressWatchResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncServerStreamingCall(method, null, options, request);
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
  public AsyncUnaryCall<global::Library.V1.PluginsGetResponse> Get(global::Library.V1.PluginsGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Library.V1.PluginsGetRequest, global::Library.V1.PluginsGetResponse>(MethodType.Unary, "library.v1.Plugins", "Get",
        Marshallers.Create<global::Library.V1.PluginsGetRequest>(value => value.ToByteArray(), bytes => global::Library.V1.PluginsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Library.V1.PluginsGetResponse>(value => value.ToByteArray(), bytes => global::Library.V1.PluginsGetResponse.Parser.ParseFrom(bytes)));
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
  public AsyncUnaryCall<global::Library.V1.Plugins.SettingsGetResponse> Get(global::Library.V1.Plugins.SettingsGetRequest request, CallOptions options = default)
  {
    var method = new Method<global::Library.V1.Plugins.SettingsGetRequest, global::Library.V1.Plugins.SettingsGetResponse>(MethodType.Unary, "library.v1.plugins.Settings", "Get",
        Marshallers.Create<global::Library.V1.Plugins.SettingsGetRequest>(value => value.ToByteArray(), bytes => global::Library.V1.Plugins.SettingsGetRequest.Parser.ParseFrom(bytes)),
        Marshallers.Create<global::Library.V1.Plugins.SettingsGetResponse>(value => value.ToByteArray(), bytes => global::Library.V1.Plugins.SettingsGetResponse.Parser.ParseFrom(bytes)));
    return invoker.AsyncUnaryCall(method, null, options, request);
  }
}
