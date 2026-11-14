# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Install and Invoke This Plugin Through the Host"""

# === Imports === #

from harmos import VISIBILITY_PUBLIC, Client, StorageTransferSettings
from harmos.client import PLUGIN_STATE_READY, PluginPlacement, PluginRemote, PluginValue
from harmos.plugin import Context, Plugin, operation

from . import Value

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Echo:
    @operation("example.echo", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def echo(self, context: Context, request: Value) -> Value:
        return request


# === Public Functions === #


async def run(arguments: list[str]) -> None:
    if arguments == ["serve"]:
        endpoint = await Plugin(Echo(), id="example", version="1.0.0").start("127.0.0.1:0")
        print(f"harmos listening {endpoint.address}", flush=True)
        await endpoint.server.wait_for_termination()
        return
    transfer = StorageTransferSettings()
    client = Client.connect(arguments[0], transfer=transfer)
    # Install and Start This Codebase's Running Instance
    await client.plugins.create("example", PluginPlacement(remote=PluginRemote(url=arguments[1])), PluginValue(enabled=True))
    watch = client.plugins.watch(name="example")
    ready = None
    async for event in watch:
        entries = event.snapshot.entries if event.HasField("snapshot") else [event.change.entry]
        ready = next((entry for entry in entries if entry.state == PLUGIN_STATE_READY), None)
        if ready is not None:
            break
    await watch.aclose()
    if ready is None:
        raise RuntimeError("Watch Plugins: closed before Ready")
    # Invoke Its Public Operation Through the Host
    contract = next(operation for operation in ready.description.declarations.operations if operation.id == "example.echo")
    data = b"hello" + bytes(transfer.chunk_bytes - len(b"hello"))
    reply = await client.invocations.invoke(contract.id, contract.fingerprint, Value(value=data).SerializeToString())
    if Value.FromString(reply).value != data:
        raise RuntimeError("Invoke example.echo: payload changed")
    print("hello", flush=True)
    await client.plugins.delete("example", ready.revision)
    await client.close()


# === Private Functions === #

# === Entry Point === #
