using Harmos.Plugin.V1;
// SPDX-License-Identifier: Apache-2.0

using System.Net;
using System.Security.Cryptography;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Harmos.V1;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Server.Kestrel.Core;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;

namespace Harmos.Sdk;

/// <summary>A plugin assembled from typed async methods, including instance methods with shared state.</summary>
public sealed class Plugin(string id, string version)
{
  private static readonly Timestamp StartedAt = Timestamp.FromDateTime(DateTime.UtcNow);
  private readonly Catalog catalog = new() { Types_ = new() };
  private readonly Dictionary<string, Handler> operations = [];
  private readonly Dictionary<string, Publication> streams = [];
  private readonly List<Func<Task>> cleanup = [];

  public Plugin Operation<TRequest, TResponse>(Operation<TRequest, TResponse> operation, Func<Context, TRequest, Task<TResponse>> handler)
      where TRequest : IMessage<TRequest>, new()
      where TResponse : IMessage<TResponse>, new()
  {
    var contract = operation.Contract.Clone();
    if (operation.IsWatch)
      throw new ArgumentException($"Register {contract.Id}: Plugin Operations Are Unary");
    if (contract.Visibility is not (Visibility.Private or Visibility.Protected or Visibility.Public))
      throw new ArgumentException($"Register {contract.Id}: Explicit Visibility Is Required");
    if (!operations.TryAdd(contract.Id, new Handler(
        payload => Sdk.Operation<TRequest, TResponse>.Decode(payload),
        async (context, request) => (await handler(context, (TRequest)request)).ToByteString(), contract)))
      throw new ArgumentException($"Duplicate Operation: {contract.Id}");
    if (contract.Visibility != Visibility.Private)
    {
      catalog.Operations.Add(contract);
      Descriptors.Merge(catalog.Types_, operation.Types);
    }
    return this;
  }

  /// <summary>Register a payload descriptor independently of executable declarations.</summary>
  public Plugin Message<T>() where T : IMessage<T>, new()
  {
    Descriptors.Merge(catalog.Types_, Descriptors.Types(new T().Descriptor));
    return this;
  }

  public Plugin Requires<TRequest, TResponse>(Operation<TRequest, TResponse> operation)
      where TRequest : IMessage<TRequest>, new()
      where TResponse : IMessage<TResponse>, new()
  {
    if (operation.Contract.Visibility == Visibility.Private)
      throw new ArgumentException($"Require {operation.Contract.Id}: Private Calls Are Local");
    if (catalog.Requires.Any(item => item.Id == operation.Contract.Id))
      throw new ArgumentException($"Duplicate Requirement: {operation.Contract.Id}");
    catalog.Requires.Add(new Requirement { Id = operation.Contract.Id, Fingerprint = operation.Contract.Fingerprint });
    Descriptors.Merge(catalog.Types_, operation.Types);
    return this;
  }

  public Plugin Observer<TRow>(string stream, Duration? interval, Func<Context, Task<TRow>> observer,
      string description = "", string intervalSetting = "") where TRow : IMessage<TRow>, new()
  {
    if (interval is null && string.IsNullOrEmpty(intervalSetting))
      throw new ArgumentException("Observer Requires an Interval or Setting");
    if (interval is not null && (interval.Seconds < 0 || interval.Nanos < 0 || interval.Nanos >= 1_000_000_000 ||
        interval.Seconds > 315_576_000_000 || (interval.Seconds == 0 && interval.Nanos == 0)))
      throw new ArgumentException("Interval Must Be a Positive Protobuf Duration", nameof(interval));
    return AddStream<TRow>(new StreamDeclaration
    {
      Id = stream,
      Interval = interval?.Clone(),
      Description = description,
      IntervalSetting = intervalSetting
    }, new Publication(async context => (await observer(context)).ToByteString(), null));
  }

  public Plugin Stream<TRow>(string stream, Func<Context, Publisher<TRow>, Task> producer, string description = "")
      where TRow : IMessage<TRow>, new()
  {
    return AddStream<TRow>(new StreamDeclaration { Id = stream, Description = description },
        new Publication(null, (context, rows) => producer(context, new Publisher<TRow>(rows, context.CancellationToken))));
  }

  public Plugin Section<T>(T value) where T : IMessage<T>
  {
    var name = value.Descriptor.FullName;
    if (catalog.Sections.Any(item => item.Type == name))
      throw new ArgumentException($"Duplicate Section: {name}");
    catalog.Sections.Add(new Section { Type = name, Payload = value.ToByteString() });
    Descriptors.Merge(catalog.Types_, Descriptors.Types(value.Descriptor));
    return this;
  }

  public Plugin OnStop(Func<Task> hook)
  {
    cleanup.Add(hook);
    return this;
  }

  public Catalog Catalog()
  {
    ArgumentException.ThrowIfNullOrWhiteSpace(id);
    ArgumentException.ThrowIfNullOrWhiteSpace(version);
    var result = catalog.Clone();
    result.Digest = ByteString.CopyFrom(SHA256.HashData(result.ToByteArray()));
    return result;
  }

  /// <summary>Serve plain gRPC. HARMOS_ADDRESS overrides address; remote mode remains alive after Stop.</summary>
  public async Task ServeAsync(string? address = null, bool remote = false, CancellationToken cancellationToken = default)
  {
    var snapshot = Catalog();
    var endpoint = IPEndPoint.Parse(Environment.GetEnvironmentVariable("HARMOS_ADDRESS") ?? address ?? "127.0.0.1:0");
    if (!remote && !IPAddress.IsLoopback(endpoint.Address))
      throw new ArgumentException("Managed Address Must Be Loopback");

    var builder = WebApplication.CreateSlimBuilder();
    builder.Logging.ClearProviders();
    builder.WebHost.ConfigureKestrel(options => options.Listen(endpoint.Address, endpoint.Port,
        listener => listener.Protocols = HttpProtocols.Http2));
    builder.Services.AddGrpc();
    builder.Services.AddSingleton(provider => new PluginService(new InstanceGetResponse
    {
      Id = id,
      Version = version,
      StartedAt = StartedAt.Clone(),
      Catalog = snapshot,
      Revision = 1,
      Sdk = "harmos_csharp",
      SdkVersion = "0.1.0"
    }, new(operations), new(streams), cleanup.ToArray(), remote, provider.GetRequiredService<IHostApplicationLifetime>()));

    await using var app = builder.Build();
    app.MapGrpcService<PluginService>();
    await app.StartAsync(cancellationToken);
    var bound = new Uri(app.Urls.Single());
    Console.WriteLine($"harmos listening {bound.Authority}");
    await Console.Out.FlushAsync(cancellationToken);
    await app.WaitForShutdownAsync(cancellationToken);
  }

  private Plugin AddStream<TRow>(StreamDeclaration declaration, Publication publication) where TRow : IMessage<TRow>, new()
  {
    ArgumentException.ThrowIfNullOrWhiteSpace(declaration.Id);
    if (!streams.TryAdd(declaration.Id, publication))
      throw new ArgumentException($"Duplicate Stream: {declaration.Id}");
    declaration.Row = new TRow().Descriptor.FullName;
    catalog.Streams.Add(declaration);
    Descriptors.Merge(catalog.Types_, Descriptors.Types(new TRow().Descriptor));
    return this;
  }
}

internal sealed record Handler(Func<ByteString, IMessage> Decode, Func<Context, IMessage, Task<ByteString>> Execute, OperationContract Contract);

internal sealed record Publication(Func<Context, Task<ByteString>>? Observer,
    Func<Context, System.Threading.Channels.ChannelWriter<Sample>, Task>? Producer);
