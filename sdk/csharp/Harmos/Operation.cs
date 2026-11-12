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
  public Operation(string id, Visibility visibility, OperationPolicy? policy = null, string description = "", bool replaySafe = false, bool watch = false)
  {
    ArgumentException.ThrowIfNullOrWhiteSpace(id);
    IsWatch = watch;
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
    if (watch)
    {
      var item = Descriptors.Fingerprint(id, new TResponse().Descriptor, new TResponse().Descriptor);
      using var hash = System.Security.Cryptography.IncrementalHash.CreateHash(System.Security.Cryptography.HashAlgorithmName.SHA256);
      hash.AppendData(System.Text.Encoding.UTF8.GetBytes("harmos.protobuf.stream.v1\0"));
      hash.AppendData(Contract.Fingerprint.Span);
      hash.AppendData([0, 1]);
      hash.AppendData(item.Span);
      Contract.Fingerprint = ByteString.CopyFrom(hash.GetHashAndReset());
    }
  }

  public OperationContract Contract { get; }
  public bool IsWatch { get; }

  internal FileDescriptorSet Types => Descriptors.Merge(Descriptors.Types(new TRequest().Descriptor), Descriptors.Types(new TResponse().Descriptor));

  internal static TRequest Decode(ByteString payload) => (TRequest)new TRequest().Descriptor.Parser.ParseFrom(payload);

  internal static TResponse DecodeResponse(ByteString payload) => (TResponse)new TResponse().Descriptor.Parser.ParseFrom(payload);
}

/// <summary>An application failure, distinct from a gRPC transport error.</summary>
public sealed class PluginFailureException(Failure failure) : Exception(failure.Message)
{
  public Failure Failure { get; } = failure.Clone();
}
