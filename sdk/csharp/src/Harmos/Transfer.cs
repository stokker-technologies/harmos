// SPDX-License-Identifier: Apache-2.0

using System;

namespace Harmos;

public static class Transfer
{
  public static int MessageBytes(StorageTransferSettings? settings = null, int? ceiling = null)
  {
    settings ??= new();
    var framing = (ulong)TransferFraming.Bytes;
    if (settings.ChunkBytes == 0 || settings.ChunkBytes > int.MaxValue - framing)
      throw new ArgumentException("Configure Transfer: chunk_bytes exceeds transport capacity");
    var required = checked((int)(settings.ChunkBytes + framing));
    var maximum = ceiling ?? checked((int)new MessageSettings().MessageBytes);
    if (maximum < required)
      throw new ArgumentException("Configure Transfer: message_bytes cannot carry chunk_bytes plus framing");
    return maximum;
  }
}
