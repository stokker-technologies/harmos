// SPDX-License-Identifier: Apache-2.0

using System.Threading.Tasks;
using Google.Protobuf.WellKnownTypes;
using Harmos.Plugins;
using Harmos;

// Serve One Typed Public Operation.
var echo = new Operation<StringValue, StringValue>("example.echo", Visibility.Public);
var plugin = new Plugin("example", "1.0.0")
    .Register(ctx => ctx.Register("prefix", new SettingValue { Text = "" }))
    .Register(ctx => ctx.Register(HostOperation.PluginsSettingsList))
    .Register(ctx => ctx.Register(echo, async (context, request) =>
    {
      var settings = await context.Host.Plugins.Settings.List();
      return new StringValue { Value = settings.Value.Values["prefix"].Text + request.Value };
    }));
await plugin.RunAsync();
