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
  public void ObserversPreserveSettingWithOrWithoutDefaultInterval()
  {
    var setting = new TimeSleepRequest { Duration = new Duration { Seconds = 2 } };
    var plugin = new Plugin("settings", "1.0.0")
        .Observer("settings.only", null, context => Task.FromResult(new TimeGetRequest()), intervalSetting: "harmos.v1.TimeSleepRequest.duration")
        .Observer("settings.default", new Duration { Seconds = 1 }, context => Task.FromResult(new TimeGetRequest()), intervalSetting: "harmos.v1.TimeSleepRequest.duration")
        .Section(setting);
    var catalog = plugin.Catalog();
    Assert.Null(catalog.Streams[0].Interval);
    Assert.Equal(TimeSleepRequest.Descriptor.FullName + ".duration", catalog.Streams[0].IntervalSetting);
    Assert.NotNull(catalog.Streams[1].Interval);
    Assert.Equal(catalog.Streams[0].IntervalSetting, catalog.Streams[1].IntervalSetting);
    Assert.Equal(setting, TimeSleepRequest.Parser.ParseFrom(Assert.Single(catalog.Sections).Payload));
    Assert.Throws<ArgumentException>(() => new Plugin("invalid", "1.0.0")
        .Observer("settings.missing", null, context => Task.FromResult(new TimeGetRequest())));
  }

  [Fact]
  public void ClockRequirementsMergeReachableTypesWithoutWireServiceMessages()
  {
    var plugin = new Plugin("clock_client", "1.0.0").Requires(Clock.Sleep).Requires(Clock.GetTime);
    var catalog = plugin.Catalog();
    var clock = Assert.Single(catalog.Types_.File, file => file.Name == "harmos/v1/plugin.proto");
    Assert.Equal(["TimeSleepRequest", "TimeSleepResponse", "TimeGetRequest", "TimeGetResponse"], clock.MessageType.Select(message => message.Name));
    Assert.Equal(["google/protobuf/duration.proto", "google/protobuf/timestamp.proto"], clock.Dependency);
    Assert.Empty(clock.Service);
    var duration = Assert.Single(catalog.Types_.File, file => file.Name == "google/protobuf/duration.proto");
    Assert.Equal(Duration.Descriptor.FullName, $"{duration.Package}.{Assert.Single(duration.MessageType).Name}");
    var timestamp = Assert.Single(catalog.Types_.File, file => file.Name == "google/protobuf/timestamp.proto");
    Assert.Equal(Timestamp.Descriptor.FullName, $"{timestamp.Package}.{Assert.Single(timestamp.MessageType).Name}");
    Assert.Equal(".google.protobuf.Duration", Assert.Single(clock.MessageType[0].Field).TypeName);
    Assert.Equal(".google.protobuf.Timestamp", Assert.Single(clock.MessageType[3].Field).TypeName);
    Assert.Equal(catalog, plugin.Catalog());

    // Hash the Independently Constructed Clock Projection in Canonical File Order.
    var request = new FileDescriptorSet
    {
      File =
      {
        duration, new FileDescriptorProto
        {
          Name = "harmos/v1/plugin.proto", Package = "harmos.v1", Syntax = "proto3", Dependency = { duration.Name },
          MessageType =
          {
            new DescriptorProto
            {
              Name = "TimeSleepRequest", Field =
              {
                new FieldDescriptorProto
                {
                  Name = "duration", Number = 1, Label = FieldDescriptorProto.Types.Label.Optional,
                  Type = FieldDescriptorProto.Types.Type.Message, TypeName = ".google.protobuf.Duration"
                }
              }
            }
          }
        }
      }
    };
    var response = new FileDescriptorSet
    {
      File = { new FileDescriptorProto
      {
        Name = "harmos/v1/plugin.proto", Package = "harmos.v1", Syntax = "proto3",
        MessageType = { new DescriptorProto { Name = "TimeSleepResponse" } }
      } }
    };
    var bytes = System.Text.Encoding.UTF8.GetBytes("harmos.time.sleep\0")
      .Concat(request.ToByteArray()).Concat(response.ToByteArray()).ToArray();
    var fingerprint = ByteString.CopyFrom(System.Security.Cryptography.SHA256.HashData(bytes));
    Assert.Equal(fingerprint, catalog.Requires[0].Fingerprint);
  }
}
