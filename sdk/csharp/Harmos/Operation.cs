// SPDX-License-Identifier: Apache-2.0

using Google.Protobuf;
using Google.Protobuf.Reflection;
using Harmos.V1;

namespace Harmos.Sdk;

/// <summary>A typed operation contract shared by providers and callers.</summary>
public sealed class Operation<TRequest, TResponse>
    where TRequest : IMessage<TRequest>, new()
    where TResponse : IMessage<TResponse>, new()
{
  public Operation(string id, Visibility visibility = Visibility.Private, OperationPolicy? policy = null, string description = "", bool replaySafe = false)
  {
    ArgumentException.ThrowIfNullOrWhiteSpace(id);
    Contract = new OperationContract
    {
      Id = id,
      Request = new TRequest().Descriptor.FullName,
      Response = new TResponse().Descriptor.FullName,
      Visibility = visibility,
      Policy = policy?.Clone(),
      ReplaySafe = replaySafe,
      Description = description,
      Fingerprint = Descriptors.Fingerprint(id, new TRequest().Descriptor, new TResponse().Descriptor)
    };
  }

  public OperationContract Contract { get; }

  internal FileDescriptorSet Types => Descriptors.Merge(Descriptors.Types(new TRequest().Descriptor), Descriptors.Types(new TResponse().Descriptor));

  internal static TRequest Decode(ByteString payload) => (TRequest)new TRequest().Descriptor.Parser.ParseFrom(payload);

  internal static TResponse DecodeResponse(ByteString payload) => (TResponse)new TResponse().Descriptor.Parser.ParseFrom(payload);
}

/// <summary>An application failure, distinct from a gRPC transport error.</summary>
public sealed class PluginFailureException(Failure failure) : Exception(failure.Message)
{
  public Failure Failure { get; } = failure.Clone();
}
