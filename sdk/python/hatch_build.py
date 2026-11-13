# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Select the Protocol Audience When Building a Python Wheel."""

# === Imports === #

# Standard Library Imports

import ast
import os
import tempfile
from pathlib import Path

from hatchling.builders.hooks.plugin.interface import BuildHookInterface


# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class CustomBuildHook(BuildHookInterface):
    def initialize(self, version, build_data):
        if self.target_name != "wheel" or version == "editable":
            return
        audience = os.environ.get("HARMOS_AUDIENCE", "plugin")
        if audience not in ("plugin", "client", "host"):
            raise ValueError(f"Build Python SDK: Unknown Audience {audience}")
        self.scratch = tempfile.TemporaryDirectory(prefix="harmos-python-audience-")
        output = Path(self.scratch.name) / "harmos"
        source = Path(self.root) / "src/harmos"
        for path in source.rglob("*"):
            if not path.is_file() or "__pycache__" in path.parts or path.suffix == ".pyc":
                continue
            relative = path.relative_to(source)
            face = relative.parts[0]
            generated = "_pb2" in path.name
            if face == "conformance" or path.name.startswith("guest_pb2"):
                continue
            if audience == "client" and (
                face in ("plugin", "context") or (not generated and path.name not in ("client.py", "resources.json", "__init__.py"))
            ):
                continue
            if audience == "plugin" and (
                path.name in ("client.py", "resources.json")
                or (face == "v1" and generated and not path.name.startswith(("types_pb2", "storage_pb2")))
            ):
                continue
            content = path.read_bytes()
            if path.name.endswith("_pb2_grpc.py"):
                if face == "context" or (audience == "plugin" and face != "plugin"):
                    continue
                text = content.decode("utf-8")
                lines = text.splitlines(keepends=True)
                remove = []
                for node in ast.parse(text).body:
                    if isinstance(node, ast.ClassDef):
                        keep = (
                            audience == "host"
                            or (audience == "plugin" and node.name.endswith("Servicer"))
                            or (audience == "client" and node.name.endswith("Stub"))
                        )
                    elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name.startswith("add_"):
                        keep = audience != "client"
                    else:
                        keep = True
                    if not keep:
                        remove.append((node.lineno - 1, node.end_lineno))
                for first, last in reversed(remove):
                    del lines[first:last]
                content = "".join(lines).encode("utf-8")
            if relative.as_posix() == "__init__.py":
                if audience == "client":
                    content = b'"""Harmos Client API."""\nfrom .client import Client\n__all__ = ["Client"]\n'
                elif audience == "plugin":
                    content = content.replace(b"from .client import Client\n", b"").replace(b'    "Client",\n', b"")
                    content = content.replace(b"from .client import Client\r\n", b"").replace(b'    "Client",\r\n', b"")
            target = output / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(content)
        self.build_config.set_exclude_all()
        build_data["force_include"][str(output)] = "harmos"

    def finalize(self, version, build_data, artifact_path):
        if hasattr(self, "scratch"):
            self.scratch.cleanup()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
