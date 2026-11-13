// SPDX-License-Identifier: Apache-2.0

using Google.Protobuf.WellKnownTypes;
using Harmos.Sdk;
using Harmos.V1;

// Serve One Typed Public Operation.
var echo = new Operation<StringValue, StringValue>("example.v1.Echo/Call", Visibility.Public);
var plugin = new Plugin("example", "1.0.0")
    .Operation(echo, (context, request) => Task.FromResult(request));
await plugin.ServeAsync();
