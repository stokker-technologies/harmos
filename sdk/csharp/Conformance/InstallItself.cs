// SPDX-License-Identifier: Apache-2.0

using Google.Protobuf;
using Grpc.Core;
using Grpc.Net.Client;
using Harmos.Sdk;
using Harmos.V1;
using Value = Harmos.Conformance.V1.Value;

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
    using var channel = GrpcChannel.ForAddress(arguments[0]);
    var client = new Client(channel.CreateCallInvoker());
    // Install and Start This Codebase's Running Instance
    await client.Plugins.Create(new PluginsCreateRequest {
      RequestId = "self-install", Name = "example",
      Placement = new PluginPlacement { Remote = new PluginRemote { Url = arguments[1] } },
      Value = new PluginValue { Enabled = true }
    });
    using var watch = client.Plugins.Watch(new PluginsWatchRequest { Name = "example" });
    PluginEntry? ready = null;
    while (ready is null && await watch.ResponseStream.MoveNext(CancellationToken.None))
    {
      var current = watch.ResponseStream.Current;
      var entries = current.Snapshot?.Entries.AsEnumerable() ?? new[] { current.Change?.Entry }.OfType<PluginEntry>();
      ready = entries.FirstOrDefault(entry => entry.State == PluginState.Ready);
    }
    if (ready is null) throw new InvalidOperationException("Watch Plugins: closed before Ready");
    // Invoke Its Public Operation Through the Host
    var contract = ready.Description.Catalog.Operations.Single(operation => operation.Id == "example.echo");
    var reply = await client.Invocations.Invoke(new InvocationsInvokeRequest {
      Operation = contract.Id, Fingerprint = contract.Fingerprint, Payload = new Value { Value_ = ByteString.CopyFromUtf8("hello") }.ToByteString()
    });
    Console.WriteLine(Value.Parser.ParseFrom(reply.Payload).Value_.ToStringUtf8());
    await client.Plugins.Delete(new PluginsDeleteRequest { Name = "example", ExpectedRevision = ready.Revision });
  }
}
