// SPDX-License-Identifier: Apache-2.0

using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Harmos.Internal.Plugin;

namespace Harmos.Plugins;

public abstract record SubscriptionItem<T> where T : IMessage<T>, new();
public sealed record StreamRow<T>(T Value, Timestamp Source, Timestamp Received) : SubscriptionItem<T> where T : IMessage<T>, new();
public sealed record Gap<T>(ulong Missed) : SubscriptionItem<T> where T : IMessage<T>, new();

/// <summary>One independent live demand in the calling context.</summary>
public sealed class Subscription<T> : IAsyncEnumerable<SubscriptionItem<T>>, IAsyncDisposable where T : IMessage<T>, new()
{
  private readonly IAsyncEnumerator<SubscriptionsSubscribeResponse> incoming;
  private readonly CancellationTokenSource cancellation;
  private Task<bool>? reading;
  private bool closed;

  internal Subscription(Context context, string stream, ulong maxItemBytes, CancellationToken cancellationToken)
  {
    var descriptor = new T().Descriptor;
    var request = new SubscriptionsSubscribeRequest
    {
      Stream = stream,
      Row = descriptor.FullName,
      Types_ = Descriptors.Types(descriptor).ToByteString()
    };
    cancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
    incoming = context.WatchAsync(HostContracts.SubscriptionsSubscribe, request, maxItemBytes, cancellation.Token).GetAsyncEnumerator();
  }

  public async Task<SubscriptionItem<T>?> NextAsync()
  {
    if (closed)
      return null;
    if (reading is not null)
      throw new InvalidOperationException("Read Subscription: Await the Outstanding Read Before Reading Again");
    reading = incoming.MoveNextAsync().AsTask();
    bool available;
    try
    {
      available = await reading;
    }
    finally
    {
      reading = null;
    }
    if (!available)
      return null;
    var response = incoming.Current;
    return response.ItemCase switch
    {
      SubscriptionsSubscribeResponse.ItemOneofCase.Missed => new Gap<T>(response.Missed),
      SubscriptionsSubscribeResponse.ItemOneofCase.Row when response.Row.Source is not null && response.Row.Received is not null =>
        new StreamRow<T>((T)new T().Descriptor.Parser.ParseFrom(response.Row.Payload), response.Row.Source, response.Row.Received),
      _ => throw new InvalidOperationException("Read Subscription: Supply a Row With Both Timestamps or a Gap")
    };
  }

  internal async Task OpenAsync()
  {
    try
    {
      if (!await incoming.MoveNextAsync() || incoming.Current.ItemCase != SubscriptionsSubscribeResponse.ItemOneofCase.Opened || !incoming.Current.Opened)
        throw new InvalidOperationException("Subscribe to Stream: Host Must Acknowledge the Admitted Demand");
    }
    catch
    {
      await CloseAsync();
      throw;
    }
  }

  public async IAsyncEnumerator<SubscriptionItem<T>> GetAsyncEnumerator(CancellationToken cancellationToken = default)
  {
    using var registration = cancellationToken.Register(cancellation.Cancel);
    try
    {
      while (await NextAsync() is { } item)
        yield return item;
    }
    finally
    {
      await CloseAsync();
    }
  }

  public async ValueTask CloseAsync()
  {
    if (closed)
      return;
    closed = true;
    var pending = reading;
    await cancellation.CancelAsync();
    if (pending is not null)
    {
      try
      {
        await pending;
      }
      catch (OperationCanceledException)
      {
      }
    }
    await incoming.DisposeAsync();
    cancellation.Dispose();
  }

  public ValueTask DisposeAsync() => CloseAsync();
}
