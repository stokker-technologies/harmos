// SPDX-License-Identifier: Apache-2.0

using System.Threading.Tasks;
using Google.Protobuf.WellKnownTypes;
using Harmos.Plugins;
using Harmos;

// Serve One Typed Public Operation.
var echo = new Operation<StringValue, StringValue>("example.echo", Visibility.Public);
var plugin = new Plugin("example", "1.0.0")
    .Operation(echo, (context, request) => Task.FromResult(request));
await plugin.ServeAsync();
