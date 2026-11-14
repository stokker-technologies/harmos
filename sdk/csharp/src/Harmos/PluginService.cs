// SPDX-License-Identifier: Apache-2.0

using System.Collections.Generic;
using System.Linq;
using System.Threading.Channels;
using System.Threading.Tasks;
using System.Threading;
using System;
using Google.Protobuf.WellKnownTypes;
using Google.Protobuf;
using Grpc.Core;
using Harmos.Internal.Plugin;
using Harmos;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Hosting;
using InstanceBase = Harmos.Internal.Plugin.Instance.InstanceBase;
using Status = Grpc.Core.Status;

namespace Harmos.Plugins;

internal sealed class PluginService(InstanceGetResponse description, Dictionary<string, Handler> operations,
    Dictionary<string, Publication> streams, Func<Task>[] cleanup, bool remote, IHostApplicationLifetime lifetime)
    : InstanceBase
{
  private readonly Dictionary<string, ByteString> requires = description.Declarations.Requires.ToDictionary(item => item.Id, item => item.Fingerprint);

  public override Task<InstanceGetResponse> Get(InstanceGetRequest request, ServerCallContext context)
  {
    if (request.Revision == 0)
      throw Invalid("Revision Must Be at Least One");
    return Task.FromResult(description.Clone());
  }

  public override async Task Invoke(IAsyncStreamReader<InstanceInvokeRequest> input, IServerStreamWriter<InstanceInvokeResponse> output, ServerCallContext call)
  {
    if (!await input.MoveNext(call.CancellationToken) || input.Current.FrameCase != InstanceInvokeRequest.FrameOneofCase.Request)
      throw Invalid("Request Must Be First");
    var request = input.Current.Request;
    if (request.Invocation.Length == 0 || request.Attempt == 0)
      throw Invalid("Invocation and Attempt Are Required");
    if (!operations.TryGetValue(request.Operation, out var handler) || handler.Contract.Visibility == Visibility.Private)
      throw new RpcException(new Status(StatusCode.NotFound, "Unknown Operation"));

    IMessage decoded;
    try
    {
      decoded = handler.Decode(request.Payload);
    }
    catch (InvalidProtocolBufferException error) { throw Invalid(error.Message); }

    await call.WriteResponseHeadersAsync(new Metadata());

    using var stopped = CancellationTokenSource.CreateLinkedTokenSource(call.CancellationToken);
    var frames = Channel.CreateUnbounded<InstanceInvokeResponse>(new UnboundedChannelOptions { SingleReader = true });
    var context = new Context(request, description.Id, requires, stopped, frames.Writer, operations);
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

  private static async Task RunInvoke(IAsyncStreamReader<InstanceInvokeRequest> input, Handler handler, IMessage request,
      Context context, ChannelWriter<InstanceInvokeResponse> output, CancellationToken transport)
  {
    Task<ByteString>? executing = null;
    Task<bool>? reading = null;
    using var readingStopped = CancellationTokenSource.CreateLinkedTokenSource(transport);
    try
    {
      executing = Execute(handler, context, request);
      var inputClosed = false;
      while (!executing.IsCompleted)
      {
        if (inputClosed)
        {
          await Task.WhenAny(executing);
          break;
        }
        reading ??= input.MoveNext(readingStopped.Token);
        await Task.WhenAny(executing, reading);
        if (executing.IsCompleted)
          break;
        if (!await reading)
        {
          if (context.HasPendingCalls)
            throw new RpcException(new Status(StatusCode.Unavailable, "Host Closed Before Child Replies"));
          inputClosed = true;
          reading = null;
          continue;
        }
        var frame = input.Current;
        switch (frame.FrameCase)
        {
          case InstanceInvokeRequest.FrameOneofCase.Child:
            await context.Reply(frame.Child);
            break;
          case InstanceInvokeRequest.FrameOneofCase.Cancel:
            await context.Cancel(frame.Cancel.Reason);
            break;
          default:
            throw Invalid("Expected Child Response or Cancel");
        }
        reading = null;
      }

      Response response;
      try
      {
        response = new Response { Payload = await executing };
      }
      catch (PluginFailureException error) { response = new Response { Failure = error.Failure }; }
      catch (OperationCanceledException) when (!transport.IsCancellationRequested)
      {
        response = new Response { Failure = context.CancellationFailure().Failure };
      }
      await context.FlushCancellations();
      await output.WriteAsync(new InstanceInvokeResponse { Response = response }, transport);
      output.TryComplete();
    }
    catch (Exception error)
    {
      output.TryComplete(error);
    }
    finally
    {
      // Release the Reader Before Kestrel Can Reuse the Request Body.
      await readingStopped.CancelAsync();
      if (reading is not null)
      {
        try
        {
          await reading;
        }
        catch (OperationCanceledException) when (readingStopped.IsCancellationRequested) { }
      }
      context.Disconnected();
      if (executing is not null)
        _ = executing.ContinueWith(task => _ = task.Exception, TaskContinuationOptions.OnlyOnFaulted);
    }
  }

  private static async Task<ByteString> Execute(Handler handler, Context context, IMessage request)
  {
    try
    {
      return await handler.Execute(context, request);
    }
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

  public override async Task Samples(IAsyncStreamReader<InstanceSamplesRequest> input, IServerStreamWriter<InstanceSamplesResponse> output, ServerCallContext call)
  {
    if (!await input.MoveNext(call.CancellationToken) || input.Current.FrameCase != InstanceSamplesRequest.FrameOneofCase.Open)
      throw Invalid("Open Must Be First");
    var open = input.Current.Open;
    if (!streams.TryGetValue(open.Stream, out var publication))
      throw new RpcException(new Status(StatusCode.NotFound, "Unknown Stream"));

    // The Host Must Receive Headers Before It Can Send the First Tick.
    await call.WriteResponseHeadersAsync(new Metadata());

    using var stopped = CancellationTokenSource.CreateLinkedTokenSource(call.CancellationToken);
    var context = new Context(new Request { Operation = open.Stream, Scope = open.Scope }, description.Id, requires, stopped);
    using var readingStopped = CancellationTokenSource.CreateLinkedTokenSource(call.CancellationToken);
    Task<bool>? reading = null;
    Task? producing = null;
    Task<ByteString>? observing = null;
    Context? tickContext = null;
    CancellationTokenSource? tickStopped = null;
    try
    {
      if (publication.Observer is not null)
      {
        ulong sequence = 0;
        ulong lastCall = 0;
        reading = input.MoveNext(readingStopped.Token);
        while (await reading)
        {
          var at = TickTime(input.Current);
          tickStopped = CancellationTokenSource.CreateLinkedTokenSource(stopped.Token);
          var frames = Channel.CreateUnbounded<InstanceInvokeResponse>(new UnboundedChannelOptions { SingleReader = true });
          tickContext = new Context(new Request { Operation = open.Stream, Scope = open.Scope, Attempt = 1 },
              description.Id, requires, tickStopped, frames.Writer, operations)
          {
            Sequence = ++sequence,
            At = at.Clone(),
            LastCall = lastCall
          };
          observing = publication.Observer(tickContext);
          reading = input.MoveNext(readingStopped.Token);
          Task<bool>? available = null;
          while (true)
          {
            if (reading.IsCompleted)
            {
              if (!await reading)
                return;
              if (input.Current.FrameCase != InstanceSamplesRequest.FrameOneofCase.Child)
                throw Invalid("Read Samples: Expected a Child Reply While a Tick Is Unanswered");
              await tickContext.Reply(input.Current.Child);
              reading = input.MoveNext(readingStopped.Token);
            }
            while (frames.Reader.TryRead(out var frame))
              await output.WriteAsync(SampleChild(frame), call.CancellationToken);
            if (observing.IsCompleted)
              break;
            if (available is null || available.IsCompleted)
              available = frames.Reader.WaitToReadAsync(tickStopped.Token).AsTask();
            await Task.WhenAny(observing, reading, available);
          }
          var row = await observing;
          await tickContext.Cancel("Tick Answered");
          await tickContext.FlushCancellations();
          while (frames.Reader.TryRead(out var frame))
            await output.WriteAsync(SampleChild(frame), call.CancellationToken);
          lastCall = tickContext.LastCall;
          var sample = new Sample { At = at, Row = row, Sequence = sequence };
          await output.WriteAsync(new InstanceSamplesResponse { Samples = { sample } }, call.CancellationToken);
          tickContext = null;
          tickStopped.Dispose();
          tickStopped = null;
        }
        return;
      }

      var rows = Channel.CreateBounded<Sample>(1);
      producing = Produce(publication, context, rows.Writer);
      reading = input.MoveNext(readingStopped.Token);
      var sampling = rows.Reader.WaitToReadAsync(call.CancellationToken).AsTask();
      while (true)
      {
        await Task.WhenAny(reading, sampling);
        if (reading.IsCompleted)
        {
          if (!await reading)
            break;
          TickTime(input.Current);
          throw Invalid("Read Samples: Device Streams Do Not Accept Ticks");
        }
        if (!await sampling)
        {
          await producing;
          if (await reading)
            throw Invalid("Read Samples: Device Streams Do Not Accept Frames");
          break;
        }
        while (rows.Reader.TryRead(out var sample))
        {
          sample.Sequence = ++context.Sequence;
          await output.WriteAsync(new InstanceSamplesResponse { Samples = { sample } }, call.CancellationToken);
        }
        sampling = rows.Reader.WaitToReadAsync(call.CancellationToken).AsTask();
      }
    }
    finally
    {
      // Demand Closure Must Also Join Any Outstanding Input Read.
      await readingStopped.CancelAsync();
      if (reading is not null)
      {
        try
        {
          await reading;
        }
        catch (OperationCanceledException) when (readingStopped.IsCancellationRequested) { }
      }
      context.Disconnected();
      tickContext?.Disconnected();
      if (producing is not null)
        await producing;
      if (observing is not null)
      {
        // Observe Completion After Demand Closure; Active Failures Already Propagate Above.
        try
        {
          await observing;
        }
        catch (Exception) when (stopped.IsCancellationRequested) { }
      }
      tickStopped?.Dispose();
    }
  }

  private static InstanceSamplesResponse SampleChild(InstanceInvokeResponse frame) => frame.FrameCase switch
  {
    InstanceInvokeResponse.FrameOneofCase.Child => new InstanceSamplesResponse { Child = frame.Child },
    InstanceInvokeResponse.FrameOneofCase.ChildCancel => new InstanceSamplesResponse { ChildCancel = frame.ChildCancel },
    InstanceInvokeResponse.FrameOneofCase.ChildDemand => new InstanceSamplesResponse { ChildDemand = frame.ChildDemand },
    _ => throw Invalid("Write Samples: Expected Child Traffic")
  };

  private static async Task Produce(Publication publication, Context context, ChannelWriter<Sample> rows)
  {
    try
    {
      await publication.Producer!(context, rows);
      rows.TryComplete();
    }
    catch (Exception error) { rows.TryComplete(error); }
  }

  private static Timestamp TickTime(InstanceSamplesRequest frame)
  {
    if (frame.FrameCase != InstanceSamplesRequest.FrameOneofCase.Tick || frame.Tick.At is null)
      throw Invalid("Read Samples: Tick Timestamp Is Required");
    ValidateTimestamp(frame.Tick.At);
    return frame.Tick.At.Clone();
  }

  internal static void ValidateTimestamp(Timestamp at)
  {
    if (at.Seconds < -62_135_596_800 || at.Seconds > 253_402_300_799 || at.Nanos < 0 || at.Nanos >= 1_000_000_000)
      throw Invalid("Invalid Timestamp");
  }

  public override async Task<InstanceStopResponse> Stop(InstanceStopRequest request, ServerCallContext context)
  {
    foreach (var hook in cleanup)
      await hook();
    if (!remote)
      context.GetHttpContext().Response.OnCompleted(() =>
      {
        lifetime.StopApplication();
        return Task.CompletedTask;
      });
    return new InstanceStopResponse();
  }

  private static RpcException Invalid(string message) => new(new Status(StatusCode.InvalidArgument, message));
}
