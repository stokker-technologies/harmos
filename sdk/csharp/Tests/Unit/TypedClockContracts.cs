// SPDX-License-Identifier: Apache-2.0

using Google.Protobuf;
using Google.Protobuf.Reflection;
using Google.Protobuf.WellKnownTypes;
using Harmos.V1;
using Xunit;
using Plugin = Harmos.Sdk.Plugin;

namespace Harmos.Sdk.Tests.Unit;

public sealed class TypedClockContracts
{
  [Fact]
  public void ClockRequirementsMergeReachableTypesWithoutWireServiceMessages()
  {
    var plugin = new Plugin("clock_client", "1.0.0").Requires(Clock.Sleep).Requires(Clock.Now);
    var catalog = plugin.Catalog();
    var clock = Assert.Single(catalog.Types_.File, file => file.Name == "harmos/v1/plugin.proto");
    Assert.Equal(["Sleep", "SleepResponse", "Now", "Time"], clock.MessageType.Select(message => message.Name));
    Assert.Equal(["google/protobuf/duration.proto", "google/protobuf/timestamp.proto"], clock.Dependency);
    Assert.Empty(clock.Service);
    var duration = Assert.Single(catalog.Types_.File, file => file.Name == "google/protobuf/duration.proto");
    Assert.Equal(Duration.Descriptor.FullName, $"{duration.Package}.{Assert.Single(duration.MessageType).Name}");
    var timestamp = Assert.Single(catalog.Types_.File, file => file.Name == "google/protobuf/timestamp.proto");
    Assert.Equal(Timestamp.Descriptor.FullName, $"{timestamp.Package}.{Assert.Single(timestamp.MessageType).Name}");
    Assert.Equal(".google.protobuf.Duration", Assert.Single(clock.MessageType[0].Field).TypeName);
    Assert.Equal(".google.protobuf.Timestamp", Assert.Single(clock.MessageType[3].Field).TypeName);
    Assert.Equal(catalog, plugin.Catalog());

    // Hash the Independently Constructed Minimal Clock Contract, Matching the Rust SDK.
    var request = new FileDescriptorSet
    {
      File =
      {
        new FileDescriptorProto
        {
          Name = "harmos/v1/plugin.proto", Package = "harmos.v1", Syntax = "proto3", Dependency = { duration.Name },
          MessageType =
          {
            new DescriptorProto
            {
              Name = "Sleep", Field =
              {
                new FieldDescriptorProto
                {
                  Name = "duration", Number = 1, Label = FieldDescriptorProto.Types.Label.Optional,
                  Type = FieldDescriptorProto.Types.Type.Message, TypeName = ".google.protobuf.Duration"
                }
              }
            }
          }
        }, duration
      }
    };
    var response = new FileDescriptorSet
    {
      File = { new FileDescriptorProto
      {
        Name = "harmos/v1/plugin.proto", Package = "harmos.v1", Syntax = "proto3",
        MessageType = { new DescriptorProto { Name = "SleepResponse" } }
      } }
    };
    var bytes = System.Text.Encoding.UTF8.GetBytes("harmos.clock.sleep\0")
      .Concat(request.ToByteArray()).Concat(response.ToByteArray()).ToArray();
    var fingerprint = ByteString.CopyFrom(System.Security.Cryptography.SHA256.HashData(bytes));
    Assert.Equal(fingerprint, catalog.Requires[0].Fingerprint);
  }
}
