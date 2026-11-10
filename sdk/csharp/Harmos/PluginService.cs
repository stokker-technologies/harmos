// SPDX-License-Identifier: Apache-2.0

using System.Threading.Channels;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Harmos.V1;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Hosting;

namespace Harmos.Sdk;

internal sealed class PluginService(Description description, Dictionary<string, Handler> operations,
    Dictionary<string, Publication> streams, Func<Task>[] cleanup, bool remote, IHostApplicationLifetime lifetime)
    : V1.Plugin.PluginBase
{
  private readonly Dictionary<string, ByteString> requires = description.Catalog.Requires.ToDictionary(item => item.Id, item => item.Fingerprint);

  public override Task<Description> Describe(DescribeRequest request, ServerCallContext context)
  {
    if (request.Revision == 0)
      throw Invalid("Revision Must Be at Least One");
    return Task.FromResult(description.Clone());
  }

  public override async Task Invoke(IAsyncStreamReader<HostFrame> input, IServerStreamWriter<PluginFrame> output, ServerCallContext call)
  {
    if (!await input.MoveNext(call.CancellationToken) || input.Current.FrameCase != HostFrame.FrameOneofCase.Request)
      throw Invalid("Request Must Be First");
    var request = input.Current.Request;
    if (request.Invocation.Length == 0 || request.Attempt == 0)
      throw Invalid("Invocation and Attempt Are Required");
    if (!operations.TryGetValue(request.Operation, out var handler))
      throw new RpcException(new Status(StatusCode.NotFound, "Unknown Operation"));

    IMessage decoded;
    try { decoded = handler.Decode(request.Payload); }
    catch (InvalidProtocolBufferException error) { throw Invalid(error.Message); }

    await call.WriteResponseHeadersAsync(new Metadata());

    using var stopped = CancellationTokenSource.CreateLinkedTokenSource(call.CancellationToken);
    var frames = Channel.CreateUnbounded<PluginFrame>(new UnboundedChannelOptions { SingleReader = true });
    var context = new Context(request, description.Id, requires, stopped, frames.Writer);
    var processing = RunInvoke(input, handler, decoded, context, frames.Writer, call.CancellationToken);
    try
    {
      await foreach (var frame in frames.Reader.ReadAllAsync(call.CancellationToken))
        await output.WriteAsync(frame, call.CancellationToken);
    }
    finally
    {
      context.Disconnected();
      await processing;
    }
  }

  private static async Task RunInvoke(IAsyncStreamReader<HostFrame> input, Handler handler, IMessage request,
      Context context, ChannelWriter<PluginFrame> output, CancellationToken transport)
  {
    Task<ByteString>? executing = null;
    try
    {
      executing = Execute(handler, context, request);
      Task<bool>? reading = input.MoveNext(transport);
      while (!executing.IsCompleted)
      {
        if (reading is null)
        {
          await Task.WhenAny(executing);
          break;
        }
        await Task.WhenAny(executing, reading);
        if (executing.IsCompleted)
          break;
        if (!await reading)
        {
          if (context.HasPendingCalls)
            throw new RpcException(new Status(StatusCode.Unavailable, "Host Closed Before Child Replies"));
          reading = null;
          continue;
        }
        var frame = input.Current;
        switch (frame.FrameCase)
        {
          case HostFrame.FrameOneofCase.Child:
            context.Reply(frame.Child);
            break;
          case HostFrame.FrameOneofCase.Cancel:
            context.Cancel(frame.Cancel.Reason);
            break;
          default:
            throw Invalid("Expected Child Response or Cancel");
        }
        reading = input.MoveNext(transport);
      }

      V1.Response response;
      try { response = new V1.Response { Payload = await executing }; }
      catch (PluginFailureException error) { response = new V1.Response { Failure = error.Failure }; }
      catch (OperationCanceledException) when (!transport.IsCancellationRequested)
      {
        response = new V1.Response { Failure = context.CancellationFailure().Failure };
      }
      await output.WriteAsync(new PluginFrame { Response = response }, transport);
      output.TryComplete();
    }
    catch (Exception error)
    {
      output.TryComplete(error);
    }
    finally
    {
      context.Disconnected();
      if (executing is not null)
        _ = executing.ContinueWith(task => _ = task.Exception, TaskContinuationOptions.OnlyOnFaulted);
    }
  }

  private static async Task<ByteString> Execute(Handler handler, Context context, IMessage request)
  {
    try { return await handler.Execute(context, request); }
    catch (PluginFailureException) { throw; }
    catch (RpcException) { throw; }
    catch (OperationCanceledException) { throw; }
    catch (Exception error)
    {
      throw new PluginFailureException(new Failure
      {
        Classification = Classification.Permanent,
        Code = "handler_failed",
        Message = error.Message,
        Action = "Inspect Plugin Diagnostics",
        Operation = context.Operation,
        Plugin = context.Plugin,
        Diagnostics = error.ToString()
      });
    }
  }

  public override async Task Publish(IAsyncStreamReader<PublishFrame> input, IServerStreamWriter<Samples> output, ServerCallContext call)
  {
    if (!await input.MoveNext(call.CancellationToken) || input.Current.FrameCase != PublishFrame.FrameOneofCase.Open)
      throw Invalid("Open Must Be First");
    var open = input.Current.Open;
    if (!streams.TryGetValue(open.Stream, out var publication))
      throw new RpcException(new Status(StatusCode.NotFound, "Unknown Stream"));

    // The Host Must Receive Headers Before It Can Send the First Tick.
    await call.WriteResponseHeadersAsync(new Metadata());

    using var stopped = CancellationTokenSource.CreateLinkedTokenSource(call.CancellationToken);
    var context = new Context(new Request { Operation = open.Stream, Scope = open.Scope }, description.Id, requires, stopped);
    Task? producing = null;
    Task<ByteString>? observing = null;
    try
    {
      if (publication.Observer is not null)
      {
        var next = input.MoveNext(call.CancellationToken);
        while (await next)
        {
          var at = TickTime(input.Current);
          context.Sequence++;
          context.At = at.Clone();
          observing = publication.Observer(context);
          next = input.MoveNext(call.CancellationToken);
          await Task.WhenAny(observing, next);
          if (next.IsCompleted && !await next)
            return;
          var row = await observing;
          var sample = new Sample { At = at, Row = row, Sequence = context.Sequence };
          await output.WriteAsync(new Samples { Samples_ = { sample } }, call.CancellationToken);
        }
        return;
      }

      var rows = Channel.CreateBounded<Sample>(1);
      producing = Produce(publication, context, rows.Writer);
      var reading = input.MoveNext(call.CancellationToken);
      var sampling = rows.Reader.WaitToReadAsync(call.CancellationToken).AsTask();
      while (true)
      {
        await Task.WhenAny(reading, sampling);
        if (reading.IsCompleted)
        {
          if (!await reading)
            break;
          TickTime(input.Current);
          throw Invalid("Device Streams Do Not Accept Ticks");
        }
        if (!await sampling)
        {
          await producing;
          if (await reading)
            throw Invalid("Device Streams Do Not Accept Frames");
          break;
        }
        while (rows.Reader.TryRead(out var sample))
        {
          sample.Sequence = ++context.Sequence;
          await output.WriteAsync(new Samples { Samples_ = { sample } }, call.CancellationToken);
        }
        sampling = rows.Reader.WaitToReadAsync(call.CancellationToken).AsTask();
      }
    }
    finally
    {
      context.Disconnected();
      if (producing is not null)
        await producing;
      if (observing is not null)
      {
        // Observe Completion After Demand Closure; Active Failures Already Propagate Above.
        try { await observing; }
        catch (Exception) when (stopped.IsCancellationRequested) { }
      }
    }
  }

  private static async Task Produce(Publication publication, Context context, ChannelWriter<Sample> rows)
  {
    try
    {
      await publication.Producer!(context, rows);
      rows.TryComplete();
    }
    catch (Exception error) { rows.TryComplete(error); }
  }

  private static Timestamp TickTime(PublishFrame frame)
  {
    if (frame.FrameCase != PublishFrame.FrameOneofCase.Tick || frame.Tick.At is null)
      throw Invalid("Tick Time Is Required");
    ValidateTimestamp(frame.Tick.At);
    return frame.Tick.At.Clone();
  }

  internal static void ValidateTimestamp(Timestamp at)
  {
    if (at.Seconds < -62_135_596_800 || at.Seconds > 253_402_300_799 || at.Nanos < 0 || at.Nanos >= 1_000_000_000)
      throw Invalid("Invalid Timestamp");
  }

  public override async Task<StopResponse> Stop(StopRequest request, ServerCallContext context)
  {
    foreach (var hook in cleanup)
      await hook();
    if (!remote)
      context.GetHttpContext().Response.OnCompleted(() =>
      {
        lifetime.StopApplication();
        return Task.CompletedTask;
      });
    return new StopResponse();
  }

  private static RpcException Invalid(string message) => new(new Status(StatusCode.InvalidArgument, message));
}
