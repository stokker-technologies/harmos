# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Install and Invoke This Plugin Through the Host"""

# === Imports === #

from harmos import Client, Context, Plugin, operation
from .v1.fixture_pb2 import Value
from harmos.v1 import plugins_pb2 as plugins, invocations_pb2 as invocations, types_pb2 as wire
from harmos.v1.storage_settings_pb2 import StorageTransferSettings

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Echo:
    @operation("example.echo", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def echo(self, context: Context, request: Value) -> Value:
        return request


# === Public Functions === #


async def run(arguments):
    if arguments == ["serve"]:
        endpoint = await Plugin(Echo(), id="example", version="1.0.0").start("127.0.0.1:0")
        print(f"harmos listening {endpoint.address}", flush=True)
        await endpoint.server.wait_for_termination()
        return
    transfer = StorageTransferSettings()
    client = Client.connect(arguments[0], transfer=transfer)
    # Install and Start This Codebase's Running Instance
    await client.plugins.create(
        plugins.PluginsCreateRequest(
            request_id="self-install",
            name="example",
            placement=plugins.PluginPlacement(remote=plugins.PluginRemote(url=arguments[1])),
            value=plugins.PluginValue(enabled=True),
        )
    )
    watch = client.plugins.watch(plugins.PluginsWatchRequest(name="example"))
    ready = None
    async for event in watch:
        entries = event.snapshot.entries if event.HasField("snapshot") else [event.change.entry]
        ready = next((entry for entry in entries if entry.state == plugins.PLUGIN_STATE_READY), None)
        if ready is not None:
            break
    watch.cancel()
    if ready is None:
        raise RuntimeError("Watch Plugins: closed before Ready")
    # Invoke Its Public Operation Through the Host
    contract = next(operation for operation in ready.description.catalog.operations if operation.id == "example.echo")
    data = b"hello" + bytes(transfer.chunk_bytes - len(b"hello"))
    reply = await client.invocations.invoke(
        invocations.InvocationsInvokeRequest(
            operation=contract.id, fingerprint=contract.fingerprint, payload=Value(value=data).SerializeToString()
        )
    )
    if Value.FromString(reply.payload).value != data:
        raise RuntimeError("Invoke example.echo: payload changed")
    print("hello", flush=True)
    await client.plugins.delete(plugins.PluginsDeleteRequest(name="example", expected_revision=ready.revision))
    await client.close()


# === Private Functions === #

# === Entry Point === #
