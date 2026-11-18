# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Explicit Inputs for Shared Resource Rendering."""

# === Imports === #

# Standard Library Imports
import keyword
from dataclasses import dataclass, field
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


@dataclass(frozen=True)
class Configuration:
    namespace: str = "harmos"
    library: FileDescriptorSet | None = None
    csharp_tools: Path | None = None
    rustfmt: Path | None = None
    python_style: Path | None = None
    csharp_namespace: str | None = None
    csharp_namespaces: dict[str, str] = field(default_factory=dict)
    client_required_fields: dict[str, set[str]] = field(default_factory=dict)
    rust_selectors: dict[str, set[str]] = field(default_factory=dict)
    python_package: str | None = None

    def __post_init__(self) -> None:
        python_package = self.python_package if self.python_package is not None else self.namespace
        identifier = python_package.isidentifier()
        reserved = keyword.iskeyword(python_package)
        if not identifier or reserved:
            raise ValueError("Generate Python SDK: Set Configuration.python_package to a Python Identifier That Is Not a Keyword")

    def library_descriptors(self) -> FileDescriptorSet:
        if self.library is not None:
            return self.library
        root = Path(__file__).resolve().parents[6]
        return FileDescriptorSet.FromString((root / "crates/harmos-protocol/src/descriptors.bin").read_bytes())


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
