// SPDX-License-Identifier: Apache-2.0

using System.Collections.Generic;
using System.Linq;
using System;
using Google.Protobuf;
using Harmos;

using var client = Client.Connect(args.FirstOrDefault() ?? "http://127.0.0.1:50051");

// Invoke an Explicitly Selected Operation with Its Discovered Contract.
if (args is [_, "--invoke", var id, var payload])
{
  var contract = (await client.Operations.Get(id)).Contract;
  var response = await client.Invocations.Create(contract.Id, contract.Fingerprint, ByteString.FromBase64(payload));
  Console.WriteLine(response.ToBase64());
  return;
}

// Connect and Discover Typed Operations.
var operations = await client.Operations.List();
foreach (var operation in operations)
  Console.WriteLine(operation.Contract.Id);
