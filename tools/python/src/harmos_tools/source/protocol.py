# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Check Protocol Caller Contracts"""

# === Imports === #

# Standard Library Imports
import subprocess
import tempfile
from pathlib import Path
from typing import Any, cast

# Third Party Imports
from google.protobuf import descriptor_pool, message_factory
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
from ..process import spawn_flags

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def caller_options(descriptors: FileDescriptorSet) -> dict[str, str]:
    """Read Custom Options Without Depending on Generated SDK Imports."""
    pool = descriptor_pool.DescriptorPool()
    pending = {file.name: file for file in descriptors.file}
    while pending:
        ready = [file for file in pending.values() if all(name not in pending for name in file.dependency)]
        if not ready:
            raise ValueError("Protocol Callers: Unresolved Descriptor Dependencies")
        for file in ready:
            pool.AddSerializedFile(file.SerializeToString())
            del pending[file.name]
    result = {}
    for file in descriptors.file:
        if "host" not in Path(file.name).parts[:-1]:
            continue
        for service in file.service:
            for method in service.method:
                identity = f"{file.package}.{service.name}/{method.name}"
                try:
                    extension = pool.FindExtensionByName("harmos.model.v1.callers")
                except KeyError as error:
                    raise ValueError(f"Protocol Callers: {identity} Has No Callers Declaration") from error
                if extension.enum_type is None:
                    raise ValueError("Protocol Callers: Option Must Be an Enum")
                options = cast(Any, message_factory.GetMessageClass(pool.FindMessageTypeByName("google.protobuf.MethodOptions"))())
                options.ParseFromString(method.options.SerializeToString())
                value = options.Extensions[extension]
                allowed = extension.enum_type.values_by_number.get(value)
                if not options.HasExtension(extension) or allowed is None or allowed.name not in {"CLIENTS", "PLUGINS", "ALL"}:
                    raise ValueError(f"Protocol Callers: {identity} Requires CLIENTS, PLUGINS or ALL")
                result[identity] = allowed.name
    return result


def narrow_callers(descriptors: FileDescriptorSet, overrides: dict[str, str]) -> None:
    """Apply Product Assembly Restrictions to the Generator's Descriptor Copy."""
    declared = caller_options(descriptors)
    audiences = {"CLIENTS": {"CLIENTS"}, "PLUGINS": {"PLUGINS"}, "ALL": {"CLIENTS", "PLUGINS"}}
    for identity, selected in overrides.items():
        original = declared.get(identity)
        if original is None or selected not in audiences or not audiences[selected] <= audiences[original]:
            raise ValueError(f"Narrow {identity}: Callers Must Be a Subset of the Declaration")
    # The Extension Number and Its Definition Are Owned by the Descriptor, Not This Tool.
    pool = descriptor_pool.DescriptorPool()
    pending = {file.name: file for file in descriptors.file}
    while pending:
        ready = [file for file in pending.values() if all(name not in pending for name in file.dependency)]
        for file in ready:
            pool.AddSerializedFile(file.SerializeToString())
            del pending[file.name]
    extension = pool.FindExtensionByName("harmos.model.v1.callers")
    if extension.enum_type is None:
        raise ValueError("Protocol Callers: Option Must Be an Enum")
    options_type = message_factory.GetMessageClass(pool.FindMessageTypeByName("google.protobuf.MethodOptions"))
    for file in descriptors.file:
        for service in file.service:
            for method in service.method:
                selected = overrides.get(f"{file.package}.{service.name}/{method.name}")
                if selected is None:
                    continue
                options = cast(Any, options_type.FromString(method.options.SerializeToString()))
                options.Extensions[extension] = extension.enum_type.values_by_name[selected].number
                method.options.ParseFromString(options.SerializeToString())


def main() -> None:
    with tempfile.TemporaryDirectory() as scratch:
        output = Path(scratch) / "descriptors.bin"
        subprocess.run(["buf", "build", "--as-file-descriptor-set", "-o", str(output)], check=True, creationflags=spawn_flags())
        methods = caller_options(FileDescriptorSet.FromString(output.read_bytes()))
        print(f"Protocol Callers: {len(methods)} Annotated Operations")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
