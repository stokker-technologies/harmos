// SPDX-License-Identifier: Apache-2.0

using System.Linq;
using System.Threading.Tasks;
using System;
using Google.Protobuf.WellKnownTypes;
using Harmos.Clients;
using Harmos.Plugins;
using Harmos.Tests.Columns;
using Harmos;
using Xunit;

namespace Harmos.Tests.Unit;

public sealed class TypedClockContracts
{
  [Fact]
  public void ObserversPreserveSettingWithOrWithoutDefaultInterval()
  {
    var setting = new SettingValue { Duration = new Duration { Seconds = 2 } };
    var minimum = new SettingValue { Duration = new Duration { Seconds = 1 } };
    var maximum = new SettingValue { Duration = new Duration { Seconds = 3 } };
    var plugin = new Harmos.Plugins.Plugin("harmos", "1.0.0")
        .Register(ctx => ctx.Register("harmos.settings.only", null, context => Task.FromResult(new TimeGetRequest()), intervalSetting: "interval"))
        .Register(ctx => ctx.Register("harmos.settings.default", new Duration { Seconds = 1 }, context => Task.FromResult(new TimeGetRequest()), intervalSetting: "interval"))
        .Register(ctx => ctx.Register("interval", setting, minimum: minimum, maximum: maximum));
    var declarations = plugin.Declarations();
    Assert.Null(declarations.Streams[0].Interval);
    Assert.Equal("interval", declarations.Streams[0].IntervalSetting);
    Assert.NotNull(declarations.Streams[1].Interval);
    Assert.Equal(declarations.Streams[0].IntervalSetting, declarations.Streams[1].IntervalSetting);
    var settings = SettingsDeclarations.Parser.ParseFrom(Assert.Single(declarations.Sections).Payload);
    Assert.Equal(setting, Assert.Single(settings.Values).Default);
    Assert.Equal(minimum, settings.Values[0].Minimum);
    Assert.Equal(maximum, settings.Values[0].Maximum);
    Assert.Throws<ArgumentException>(() => new Harmos.Plugins.Plugin("invalid", "1.0.0")
        .Register(ctx => ctx.Register("settings.missing", null, context => Task.FromResult(new TimeGetRequest()))));
  }

  [Fact]
  public void WindowObserversDeclareWindowSettingsAndMirroredOneValueRows()
  {
    var declarations = new Harmos.Plugins.Plugin("tests", "1.0.0")
        .Register(ctx => ctx.Register("tests.windows.sample", new Duration { Seconds = 1 }, context => Task.FromResult(new Reading()), windowSetting: "window"))
        .Register(ctx => ctx.Register<Reading, ReadingColumns>("tests.windows.columns", new Duration { Seconds = 1 },
            (context, window) => Task.FromResult(new ReadingColumns()), windowSetting: "window"))
        .Declarations();
    Assert.Equal(["window", "window"], declarations.Streams.Select(stream => stream.WindowSetting));
    Assert.Equal([Reading.Descriptor.FullName, Reading.Descriptor.FullName], declarations.Streams.Select(stream => stream.Row));

    // Columns Repeat Every Row Field with the Same Number and Type.
    Assert.Throws<ArgumentException>(() => new Harmos.Plugins.Plugin("tests", "1.0.0").Register(ctx => ctx.Register<Reading, RetypedColumns>(
        "tests.windows.retyped", new Duration { Seconds = 1 }, (context, window) => Task.FromResult(new RetypedColumns()))));
    Assert.Throws<ArgumentException>(() => new Harmos.Plugins.Plugin("tests", "1.0.0").Register(ctx => ctx.Register<Reading, PartialColumns>(
        "tests.windows.partial", new Duration { Seconds = 1 }, (context, window) => Task.FromResult(new PartialColumns()))));

    // Interval Rows Hold One Value per Field in Both Forms.
    Assert.Throws<ArgumentException>(() => new Harmos.Plugins.Plugin("tests", "1.0.0")
        .Register(ctx => ctx.Register("tests.windows.listed", new Duration { Seconds = 1 }, context => Task.FromResult(new Listed()))));
    Assert.Throws<ArgumentException>(() => new Harmos.Plugins.Plugin("tests", "1.0.0")
        .Register(ctx => ctx.Register("tests.windows.present", new Duration { Seconds = 1 }, context => Task.FromResult(new Present()))));
    Assert.Throws<ArgumentException>(() => new Harmos.Plugins.Plugin("tests", "1.0.0")
        .Register(ctx => ctx.Register("tests.windows.choice", new Duration { Seconds = 1 }, context => Task.FromResult(new Choice()))));
    Assert.Throws<ArgumentException>(() => new Harmos.Plugins.Plugin("tests", "1.0.0").Register(ctx => ctx.Register<Listed, Listed>(
        "tests.windows.listed", new Duration { Seconds = 1 }, (context, window) => Task.FromResult(new Listed()))));
  }

  [Fact]
  public void ClockRequirementsMergeReachableTypesWithoutWireServiceMessages()
  {
    var plugin = new Harmos.Plugins.Plugin("clock_client", "1.0.0").Register(ctx => ctx.Register(HostOperation.TimeSleep)).Register(ctx => ctx.Register(HostOperation.TimeGet));
    var declarations = plugin.Declarations();
    var clock = Assert.Single(declarations.Types_.File, file => file.Name == "harmos/host/v1/time.proto");
    Assert.Empty(clock.Service);
    var duration = Assert.Single(declarations.Types_.File, file => file.Name == "google/protobuf/duration.proto");
    Assert.Equal(Duration.Descriptor.FullName, $"{duration.Package}.{Assert.Single(duration.MessageType).Name}");
    var timestamp = Assert.Single(declarations.Types_.File, file => file.Name == "google/protobuf/timestamp.proto");
    Assert.Equal(Timestamp.Descriptor.FullName, $"{timestamp.Package}.{Assert.Single(timestamp.MessageType).Name}");
    Assert.Equal(".google.protobuf.Duration", Assert.Single(clock.MessageType.Single(message => message.Name == "TimeSleepRequest").Field).TypeName);
    Assert.Equal(".google.protobuf.Timestamp", Assert.Single(clock.MessageType.Single(message => message.Name == "TimeGetResponse").Field).TypeName);
    Assert.Equal(declarations, plugin.Declarations());

  }
}
