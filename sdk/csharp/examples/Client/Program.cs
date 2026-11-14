// SPDX-License-Identifier: Apache-2.0

using System.Collections.Generic;
using System.Linq;
using System;
using Harmos;
using var client = Client.Connect(args.FirstOrDefault() ?? "http://127.0.0.1:50051");

// Connect and Discover Typed Operations.
var operations = await client.Operations.List();
foreach (var operation in operations)
  Console.WriteLine(operation.Contract.Id);
