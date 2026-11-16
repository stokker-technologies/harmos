// SPDX-License-Identifier: Apache-2.0

using System.Collections.Generic;
using System.Linq;
using System.Net;
using System.Security.Cryptography;
using System.Threading.Channels;
using System.Threading.Tasks;
using System.Threading;
using System;
using Google.Protobuf.WellKnownTypes;
using Google.Protobuf;
using Harmos.Internal.Plugin;
using Harmos;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Server.Kestrel.Core;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;

namespace Harmos.Plugins;

/// <summary>A plugin assembled from typed async methods, including instance methods with shared state.</summary>
public sealed class Plugin(string id, string version, StorageTransferSettings? transfer = null)
{
  private readonly StorageTransferSettings transferSettings = transfer?.Clone() ?? new();
  private static readonly Timestamp StartedAt = Timestamp.FromDateTime(DateTime.UtcNow);
  private readonly Declarations declarations = new() { Types_ = new() };
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
        payload => operation.Decode(payload),
        async (context, request) => (await handler(context, (TRequest)request)).ToByteString(), contract)))
      throw new ArgumentException($"Duplicate Operation: {contract.Id}");
    if (contract.Visibility != Visibility.Private)
    {
      declarations.Operations.Add(contract);
      Descriptors.Merge(declarations.Types_, operation.Types);
    }
    return this;
  }

  /// <summary>Register a payload descriptor independently of executable declarations.</summary>
  public Plugin Message<T>() where T : IMessage<T>, new()
  {
    Descriptors.Merge(declarations.Types_, Descriptors.Types(new T().Descriptor));
    return this;
  }

  public Plugin Requires<TRequest, TResponse>(Operation<TRequest, TResponse> operation)
      where TRequest : IMessage<TRequest>, new()
      where TResponse : IMessage<TResponse>, new()
  {
    if (operation.Contract.Visibility == Visibility.Private)
      throw new ArgumentException($"Require {operation.Contract.Id}: Private Calls Are Local");
    if (declarations.Requires.Any(item => item.Id == operation.Contract.Id))
      throw new ArgumentException($"Duplicate Requirement: {operation.Contract.Id}");
    declarations.Requires.Add(new Requirement { Id = operation.Contract.Id, Fingerprint = operation.Contract.Fingerprint });
    Descriptors.Merge(declarations.Types_, operation.Types);
    return this;
  }

  /// <summary>Declare an interval stream answered one sample at a time, in order, for every sample of each host window.</summary>
  public Plugin Observer<TRow>(string stream, Duration? interval, Func<Context, Task<TRow>> observer,
      string description = "", string intervalSetting = "", string windowSetting = "") where TRow : IMessage<TRow>, new()
  {
    var row = new TRow().Descriptor;
    var fields = Columns.RowFields(row, stream);
    return AddInterval<TRow>(stream, interval, description, intervalSetting, windowSetting, async (context, window) =>
    {
      var rows = new IMessage[Columns.Capacity(window, stream)];
      for (var index = 0; index < rows.Length; index++)
      {
        // Demand Closure Stops the Window Before Its Remaining Samples.
        context.CancellationToken.ThrowIfCancellationRequested();
        rows[index] = await observer(context.Sample(window.Sequence + (ulong)index, window.SampleTime((ulong)index)))
            ?? throw Columns.Failed($"Observe {stream}: Return a Row for Every Sample");
      }
      return Columns.FromRows(row, fields, rows);
    });
  }

  /// <summary>Declare an interval stream whose observer answers each host window with its columns message.</summary>
  /// <remarks>TColumns repeats every TRow field with the same number and type and holds one value per sample.</remarks>
  public Plugin WindowObserver<TRow, TColumns>(string stream, Duration? interval, Func<Context, Window, Task<TColumns>> observer,
      string description = "", string intervalSetting = "", string windowSetting = "")
      where TRow : IMessage<TRow>, new()
      where TColumns : IMessage<TColumns>, new()
  {
    var row = new TRow().Descriptor;
    var fields = Columns.Mirror(row, new TColumns().Descriptor, stream);
    return AddInterval<TRow>(stream, interval, description, intervalSetting, windowSetting, async (context, window) =>
        Columns.FromColumns(row, fields, await observer(context, window)
            ?? throw Columns.Failed($"Observe {stream}: Return the Window's Columns"), window, stream));
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
    if (declarations.Sections.Any(item => item.Type == name))
      throw new ArgumentException($"Duplicate Section: {name}");
    declarations.Sections.Add(new Section { Type = name, Payload = value.ToByteString() });
    Descriptors.Merge(declarations.Types_, Descriptors.Types(value.Descriptor));
    return this;
  }

  /// <summary>Declare a library-stored setting with an explicit typed default.</summary>
  public Plugin Setting(string name, SettingValue defaultValue, string title = "", string description = "")
  {
    ArgumentException.ThrowIfNullOrWhiteSpace(name);
    ArgumentNullException.ThrowIfNull(defaultValue);
    if (defaultValue.ValueCase == SettingValue.ValueOneofCase.None)
      throw new ArgumentException($"Declare Setting {name}: Select a Typed Default", nameof(defaultValue));
    var section = declarations.Sections.SingleOrDefault(item => item.Type == SettingsDeclarations.Descriptor.FullName);
    var settings = section is null ? new SettingsDeclarations() : SettingsDeclarations.Parser.ParseFrom(section.Payload);
    if (settings.Values.Any(item => item.Name == name))
      throw new ArgumentException($"Declare Setting {name}: Duplicate Name", nameof(name));
    settings.Values.Add(new SettingDeclaration
    {
      Name = name,
      Title = title,
      Description = description,
      Default = defaultValue.Clone()
    });
    if (section is null)
      return Section(settings);
    section.Payload = settings.ToByteString();
    return this;
  }

  public Plugin OnStop(Func<Task> hook)
  {
    cleanup.Add(hook);
    return this;
  }

  public Declarations Declarations()
  {
    ArgumentException.ThrowIfNullOrWhiteSpace(id);
    ArgumentException.ThrowIfNullOrWhiteSpace(version);
    var result = declarations.Clone();
    result.Digest = ByteString.CopyFrom(SHA256.HashData(result.ToByteArray()));
    return result;
  }

  /// <summary>Serve plain gRPC. HARMOS_ADDRESS overrides address; remote mode remains alive after Stop.</summary>
  public async Task ServeAsync(string? address = null, bool remote = false, CancellationToken cancellationToken = default)
  {
    var snapshot = Declarations();
    var endpoint = IPEndPoint.Parse(Environment.GetEnvironmentVariable("HARMOS_ADDRESS") ?? address ?? "127.0.0.1:0");
    if (!remote && !IPAddress.IsLoopback(endpoint.Address))
      throw new ArgumentException("Managed Address Must Be Loopback");

    var builder = WebApplication.CreateSlimBuilder();
    builder.Logging.ClearProviders();
    builder.WebHost.ConfigureKestrel(options => options.Listen(endpoint.Address, endpoint.Port,
        listener => listener.Protocols = HttpProtocols.Http2));
    var maximum = Transfer.MessageBytes(transferSettings);
    builder.Services.AddGrpc(options => { options.MaxReceiveMessageSize = maximum; options.MaxSendMessageSize = maximum; });
    builder.Services.AddSingleton(provider => Service(snapshot, (ulong)maximum, remote, provider.GetRequiredService<IHostApplicationLifetime>()));

    await using var app = builder.Build();
    app.MapGrpcService<PluginService>();
    await app.StartAsync(cancellationToken);
    var bound = new Uri(app.Urls.Single());
    Console.WriteLine($"harmos listening {bound.Authority}");
    await Console.Out.FlushAsync(cancellationToken);
    await app.WaitForShutdownAsync(cancellationToken);
  }

  /// <summary>The served instance; message_bytes bounds every response, including each window part.</summary>
  internal PluginService Service(Declarations snapshot, ulong messageBytes, bool remote, IHostApplicationLifetime lifetime) => new(new InstanceGetResponse
  {
    Id = id,
    Version = version,
    StartedAt = StartedAt.Clone(),
    Declarations = snapshot,
    Revision = 1,
    Sdk = "harmos_csharp",
    SdkVersion = "0.1.0",
    MessageBytes = messageBytes
  }, new(operations), new(streams), cleanup.ToArray(), remote, lifetime);

  private Plugin AddInterval<TRow>(string stream, Duration? interval, string description, string intervalSetting, string windowSetting,
      Func<Context, Window, Task<Columns>> observer) where TRow : IMessage<TRow>, new()
  {
    if (interval is null && string.IsNullOrEmpty(intervalSetting))
      throw new ArgumentException($"Declare {stream}: Give an Interval or Interval Setting");
    if (interval is not null && (interval.Seconds < 0 || interval.Nanos < 0 || interval.Nanos >= 1_000_000_000 ||
        interval.Seconds > 315_576_000_000 || (interval.Seconds == 0 && interval.Nanos == 0)))
      throw new ArgumentException($"Declare {stream}: Give a Positive Protobuf Duration Interval", nameof(interval));
    return AddStream<TRow>(new StreamDeclaration
    {
      Id = stream,
      Interval = interval?.Clone(),
      Description = description,
      IntervalSetting = intervalSetting,
      WindowSetting = windowSetting
    }, new Publication(observer, null));
  }

  private Plugin AddStream<TRow>(StreamDeclaration declaration, Publication publication) where TRow : IMessage<TRow>, new()
  {
    ArgumentException.ThrowIfNullOrWhiteSpace(declaration.Id);
    if (!streams.TryAdd(declaration.Id, publication))
      throw new ArgumentException($"Duplicate Stream: {declaration.Id}");
    declaration.Row = new TRow().Descriptor.FullName;
    declarations.Streams.Add(declaration);
    Descriptors.Merge(declarations.Types_, Descriptors.Types(new TRow().Descriptor));
    return this;
  }
}

internal sealed record Handler(Func<ByteString, IMessage> Decode, Func<Context, IMessage, Task<ByteString>> Execute, OperationContract Contract);

internal sealed record Publication(Func<Context, Window, Task<Columns>>? Observer,
    Func<Context, ChannelWriter<Sample>, Task>? Producer);
