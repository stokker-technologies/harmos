// SPDX-License-Identifier: Apache-2.0

using Harmos.Plugin.Host.V1;
using Google.Protobuf;
using Google.Protobuf.Reflection;
using Google.Protobuf.WellKnownTypes;
using Harmos.Sdk;
using Harmos.V1;
using SdkPlugin = Harmos.Sdk.Plugin;
using Xunit;

namespace Harmos.Tests.Unit;

public sealed class TypedClockContracts
{
  [Fact]
  public void ObserversPreserveSettingWithOrWithoutDefaultInterval()
  {
    var setting = new SettingValue { Duration = new Duration { Seconds = 2 } };
    var plugin = new SdkPlugin("settings", "1.0.0")
        .Observer("settings.only", null, context => Task.FromResult(new TimeGetRequest()), intervalSetting: "interval")
        .Observer("settings.default", new Duration { Seconds = 1 }, context => Task.FromResult(new TimeGetRequest()), intervalSetting: "interval")
        .Setting("interval", setting);
    var catalog = plugin.Catalog();
    Assert.Null(catalog.Streams[0].Interval);
    Assert.Equal("interval", catalog.Streams[0].IntervalSetting);
    Assert.NotNull(catalog.Streams[1].Interval);
    Assert.Equal(catalog.Streams[0].IntervalSetting, catalog.Streams[1].IntervalSetting);
    var declarations = SettingsDeclarations.Parser.ParseFrom(Assert.Single(catalog.Sections).Payload);
    Assert.Equal(setting, Assert.Single(declarations.Values).Default);
    Assert.Throws<ArgumentException>(() => new SdkPlugin("invalid", "1.0.0")
        .Observer("settings.missing", null, context => Task.FromResult(new TimeGetRequest())));
  }

  [Fact]
  public void ClockRequirementsMergeReachableTypesWithoutWireServiceMessages()
  {
    var plugin = new SdkPlugin("clock_client", "1.0.0").Requires(Clock.Sleep).Requires(Clock.GetTime);
    var catalog = plugin.Catalog();
    var clock = Assert.Single(catalog.Types_.File, file => file.Name == "harmos/plugin/host/v1/time.proto");
    Assert.Equal(["TimeSleepRequest", "TimeSleepResponse", "TimeGetRequest", "TimeGetResponse"], clock.MessageType.Select(message => message.Name));
    Assert.Equal(["google/protobuf/duration.proto", "google/protobuf/timestamp.proto"], clock.Dependency);
    Assert.Empty(clock.Service);
    var duration = Assert.Single(catalog.Types_.File, file => file.Name == "google/protobuf/duration.proto");
    Assert.Equal(Duration.Descriptor.FullName, $"{duration.Package}.{Assert.Single(duration.MessageType).Name}");
    var timestamp = Assert.Single(catalog.Types_.File, file => file.Name == "google/protobuf/timestamp.proto");
    Assert.Equal(Timestamp.Descriptor.FullName, $"{timestamp.Package}.{Assert.Single(timestamp.MessageType).Name}");
    Assert.Equal(".google.protobuf.Duration", Assert.Single(clock.MessageType.Single(message => message.Name == "TimeSleepRequest").Field).TypeName);
    Assert.Equal(".google.protobuf.Timestamp", Assert.Single(clock.MessageType.Single(message => message.Name == "TimeGetResponse").Field).TypeName);
    Assert.Equal(catalog, plugin.Catalog());

    // Hash the Independently Constructed Clock Projection in Canonical File Order.
    var request = new FileDescriptorSet
    {
      File =
      {
        duration, new FileDescriptorProto
        {
          Name = "harmos/plugin/host/v1/time.proto", Package = "harmos.plugin.host.v1", Syntax = "proto3", Dependency = { duration.Name },
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
        Name = "harmos/plugin/host/v1/time.proto", Package = "harmos.plugin.host.v1", Syntax = "proto3",
        MessageType = { new DescriptorProto { Name = "TimeSleepResponse" } }
      } }
    };
    var bytes = System.Text.Encoding.UTF8.GetBytes("harmos.plugin.host.v1.Time/Sleep\0")
      .Concat(request.ToByteArray()).Concat(response.ToByteArray()).ToArray();
    var fingerprint = ByteString.CopyFrom(System.Security.Cryptography.SHA256.HashData(bytes));
    Assert.Equal(fingerprint, catalog.Requires[0].Fingerprint);
  }
}
