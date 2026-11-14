// SPDX-License-Identifier: Apache-2.0

using System;

namespace Harmos;

/// <summary>An application failure, distinct from a gRPC transport error.</summary>
public sealed class PluginFailureException(Failure failure) : Exception(failure.Message)
{
  public Failure Failure { get; } = failure.Clone();
}
