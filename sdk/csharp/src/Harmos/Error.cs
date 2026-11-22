// SPDX-License-Identifier: Apache-2.0

using System;
using System.Collections.Generic;

namespace Harmos;

/// <summary>An operation failure with the host scheduler's retry evidence.</summary>
public sealed class OperationException(Failure failure) : Exception(failure.Message)
{
  public Failure Failure { get; } = failure.Clone();
  public string Kind => Failure.Classification switch
  {
    Classification.Transient => "transient",
    Classification.Refused => "refused",
    Classification.Invalid => "invalid",
    Classification.Cancelled or Classification.TimedOut => "cancelled",
    _ => "permanent"
  };
  public string Code => Failure.Code;
  public string Action => Failure.Action;
  public string Operation => Failure.Operation;
  public OperationRetry Retry => Failure.Retry?.Clone() ?? new();

  public Dictionary<string, object> ToDictionary() => new()
  {
    ["kind"] = Kind,
    ["code"] = Code,
    ["message"] = Message,
    ["action"] = Action,
    ["operation"] = Operation,
    ["retry"] = new Dictionary<string, object>
    {
      ["retryable"] = Retry.Retryable,
      ["attempts"] = Retry.Attempts,
      ["max_attempts"] = Retry.MaxAttempts,
      ["next_after_s"] = Retry.NextAfterS
    }
  };
}
