// SPDX-License-Identifier: Apache-2.0

using System.Linq;
using System.Threading.Tasks;
using System;
using Google.Protobuf;
using Grpc.Core;
using Harmos.Clients;
using Harmos.Plugins;
using Harmos;
using Value = Harmos.Conformance.Value;

internal static class InstallItself
{
  public static async Task Run(string[] arguments)
  {
    if (arguments is ["serve"])
    {
      await new Plugin("example", "1.0.0")
          .Operation(new Operation<Value, Value>("example.echo", Visibility.Public), (_, request) => Task.FromResult(request))
          .ServeAsync("127.0.0.1:0", remote: true);
      return;
    }
    var transfer = new StorageTransferSettings();
    using var client = Client.Connect(arguments[0], transfer);
    // Install and Start This Codebase's Running Instance
    await client.Plugins.Create("example", new PluginPlacement { Remote = new PluginRemote { Url = arguments[1] } },
        new PluginValue { Enabled = true });
    await using var watch = client.Plugins.Watch(name: "example").GetAsyncEnumerator();
    PluginEntry? ready = null;
    while (ready is null && await watch.MoveNextAsync())
    {
      var current = watch.Current;
      var entries = current.Snapshot?.Entries.AsEnumerable() ?? new[] { current.Change?.Entry }.OfType<PluginEntry>();
      ready = entries.FirstOrDefault(entry => entry.State == PluginState.Ready);
    }
    if (ready is null)
      throw new InvalidOperationException("Watch Plugins: closed before Ready");
    // Invoke Its Public Operation Through the Host
    var contract = ready.Description.Declarations.Operations.Single(operation => operation.Id == "example.echo");
    var data = new byte[checked((int)transfer.ChunkBytes)];
    "hello"u8.CopyTo(data);
    var reply = await client.Invocations.Invoke(contract.Id, contract.Fingerprint, new Value { Value_ = ByteString.CopyFrom(data) }.ToByteString());
    if (!Value.Parser.ParseFrom(reply).Value_.Span.SequenceEqual(data))
      throw new InvalidOperationException("Invoke example.echo: payload changed");
    Console.WriteLine("hello");
    await client.Plugins.Delete("example", ready.Revision);
  }
}
