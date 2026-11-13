// SPDX-License-Identifier: Apache-2.0

namespace Harmos.Sdk;

public static class Transfer
{
  public static int MessageBytes(global::Harmos.Storage.V1.StorageTransferSettings? settings = null, int? ceiling = null)
  {
    settings ??= new();
    var framing = (ulong)global::Harmos.Storage.V1.TransferFraming.Bytes;
    if (settings.ChunkBytes == 0 || settings.ChunkBytes > int.MaxValue - framing)
      throw new ArgumentException("Configure Transfer: StorageSettings.transfer.chunk_bytes exceeds transport capacity");
    var required = checked((int)(settings.ChunkBytes + framing));
    var maximum = ceiling ?? checked((int)new global::Harmos.Storage.V1.MessageSettings().MessageBytes);
    if (maximum < required) throw new ArgumentException("Configure Transfer: message_bytes cannot carry StorageSettings.transfer.chunk_bytes plus framing");
    return maximum;
  }
}
