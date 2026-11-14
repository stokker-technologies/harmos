// SPDX-License-Identifier: Apache-2.0

using System.Threading.Channels;
using System.Threading.Tasks;
using System.Threading;
using Google.Protobuf.WellKnownTypes;
using Google.Protobuf;
using Harmos;

namespace Harmos.Plugins;

/// <summary>A typed device stream publisher. Demand closure cancels pending writes.</summary>
public sealed class Publisher<TRow> where TRow : IMessage<TRow>
{
  private readonly ChannelWriter<Sample> rows;
  private readonly CancellationToken cancellationToken;

  internal Publisher(ChannelWriter<Sample> rows, CancellationToken cancellationToken)
  {
    this.rows = rows;
    this.cancellationToken = cancellationToken;
  }

  public ValueTask PublishAsync(TRow row, Timestamp? at = null)
  {
    if (at is not null)
      PluginService.ValidateTimestamp(at);
    var sample = new Sample { Row = row.ToByteString(), At = at?.Clone() };
    return rows.WriteAsync(sample, cancellationToken);
  }
}
