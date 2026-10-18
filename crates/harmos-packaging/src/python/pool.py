# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Producer-Private Descriptors With Standard Protobuf Message Classes"""

# === Imports === #

# Standard Library Imports
from typing import Any

# Third Party Imports
from google.protobuf.descriptor import Descriptor, FileDescriptor
from google.protobuf.descriptor_pool import DescriptorPool
from google.protobuf.message import Message
from google.protobuf.message_factory import GetMessageClass

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

POOL = DescriptorPool()

# === Types === #

# === Classes === #

# === Public Functions === #


def build(file: FileDescriptor, module_name: str, namespace: dict[str, Any]) -> None:
    # Use Protobuf's Public Factory Without Its Process-Global Symbol Database
    for name, descriptor in file.message_types_by_name.items():
        cls = GetMessageClass(descriptor)
        _names(cls, descriptor, module_name, name)
        namespace[name] = cls
    for name, descriptor in file.services_by_name.items():
        namespace["_" + name.upper()] = descriptor


# === Private Functions === #


def _names(cls: type[Message], descriptor: Descriptor, module: str, name: str) -> None:
    cls.__module__ = module
    cls.__qualname__ = name
    for nested in descriptor.nested_types:
        nested_class = GetMessageClass(nested)
        _names(nested_class, nested, module, name + "." + nested.name)


# === Entry Point === #
