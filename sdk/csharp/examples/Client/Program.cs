// SPDX-License-Identifier: Apache-2.0

using Harmos.Client.V1;
using Harmos.Sdk;

// Connect and Discover Typed Operations.
using var client = Client.Connect(args.FirstOrDefault() ?? "http://127.0.0.1:50051");
var operations = await client.Operations.List(new OperationsListRequest());
foreach (var operation in operations.Operations)
  Console.WriteLine(operation.Contract.Id);
