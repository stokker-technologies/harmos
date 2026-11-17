# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Generate the Composed Client, Plugin Context and Public Payload Barrels."""

# === Imports === #

# Standard Library Imports
import json
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from dataclasses import replace
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
from harmos_tools.build import csharp_protocol
from harmos_tools.process import spawn_flags
from harmos_tools.source.protocol import narrow_callers

from . import cpp, csharp, descriptor, python, rust
from .configuration import Configuration

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def generate(check: bool = False, *, root: Path, descriptors: Path, composition: Path, configuration: Configuration) -> None:
    if re.fullmatch(r"[a-z][a-z0-9_]*", configuration.namespace) is None:
        raise ValueError("Generate SDK: Supply One Lowercase Package Name in Configuration.namespace")
    source = root
    output = Path(tempfile.mkdtemp())
    try:
        _render(source, output, descriptors, composition, configuration)
        if check:
            differences = compare(source, output, configuration)
            if differences:
                raise ValueError("Check SDK Definitions: Run mise run generate -- --protocol\n" + "\n".join(differences))
            return
        expected = inventory(output, configuration)
        existing = inventory(source, configuration)
        for relative in existing - expected:
            (source / relative).unlink()
        for relative in sorted(expected):
            destination = source / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((output / relative).read_bytes())
    finally:
        shutil.rmtree(output)


def inventory(root: Path, configuration: Configuration) -> set[Path]:
    """Collect This Generator's Complete Output Inventory, Including Retired Files."""
    patterns = (
        f"sdk/python/src/{configuration.namespace}/_generated/**/*.py",
        f"sdk/python/src/{configuration.namespace}/_generated/**/*.pyi",
        f"sdk/csharp/src/{configuration.namespace.title()}/Generated/**/*.cs",
        f"sdk/cpp/include/{configuration.namespace}/_generated/**/*.hpp",
        "sdk/cpp/src/generated/*.cpp",
    )
    files = set()
    for pattern in patterns:
        for path in root.glob(pattern):
            if path.is_file():
                files.add(path.relative_to(root))
    names = (
        "sdk/rust/src/exports.rs",
        "sdk/rust/src/host.rs",
        "sdk/rust/src/client/resources.rs",
        "sdk/rust/src/client/types.rs",
        "sdk/rust/src/plugin/host.rs",
        "sdk/rust/src/plugin/host_exports.rs",
        "sdk/rust/src/plugin/types.rs",
        f"sdk/python/src/{configuration.namespace}/__init__.py",
        f"sdk/python/src/{configuration.namespace}/client.py",
        f"sdk/python/src/{configuration.namespace}/plugin/__init__.py",
        f"sdk/cpp/include/{configuration.namespace}/messages.hpp",
        f"sdk/cpp/include/{configuration.namespace}/detail/messages.hpp",
    )
    for name in names:
        if (root / name).is_file():
            files.add(Path(name))
    for path in (root / "sdk/rust/src").rglob("*.rs"):
        if path.read_bytes().startswith(b"// @generated from protocol descriptors; do not edit."):
            files.add(path.relative_to(root))
    return files


def compare(source: Path, expected: Path, configuration: Configuration) -> list[str]:
    """Report Missing, Changed and Obsolete Generated Files Without Writing."""
    wanted = inventory(expected, configuration)
    existing = inventory(source, configuration)
    differences = []
    for path in sorted(wanted - existing):
        differences.append(f"Missing: {path.as_posix()}")
    for path in sorted(existing - wanted):
        differences.append(f"Obsolete: {path.as_posix()}")
    for path in sorted(wanted & existing):
        if (source / path).read_bytes() != (expected / path).read_bytes():
            differences.append(f"Changed: {path.as_posix()}")
    return differences


# === Private Functions === #


def _render(source: Path, root: Path, descriptor_path: Path, composition: Path, configuration: Configuration) -> None:
    csharp_tools = configuration.csharp_tools
    if csharp_tools is None:
        raise ValueError("Generate SDK: Supply the Restored Grpc.Tools Directory in Configuration.csharp_tools")
    settings = tomllib.loads(composition.read_text(encoding="utf-8"))
    descriptors = FileDescriptorSet.FromString(descriptor_path.read_bytes())
    present = set()
    for file in descriptors.file:
        present.add(file.package)
    for library in settings["libraries"]:
        library_source = (composition.parent / library["descriptors"]).resolve()
        if library_source != descriptor_path.resolve():
            raise ValueError("Compose SDK: Declare the Vendored Descriptor Artifact")
        if set(library["packages"]) - present:
            raise ValueError("Compose SDK: Add the Missing Library Package to the Vendored Descriptors")
    configuration = replace(
        configuration,
        library=configuration.library
        if configuration.library is not None
        else FileDescriptorSet.FromString(descriptors.SerializeToString()),
    )
    narrow_callers(descriptors, settings.get("callers", {}))
    methods = descriptor.table(descriptors)
    clients = []
    context = []
    for method in methods:
        if method["face"] == "client":
            method = method.copy()
            required = configuration.client_required_fields.get(method["request"], set())
            method["fields"] = [
                descriptor.Field(**{**item, "optional": False}) if item["name"] in required else item for item in method["fields"]
            ]
            clients.append(method)
        if method["face"] == "context":
            context.append(method)
    for folder in (
        "sdk/rust/src/client",
        "sdk/rust/src/plugin",
        f"sdk/python/src/{configuration.namespace}/_generated",
        f"sdk/python/src/{configuration.namespace}/plugin",
        f"sdk/csharp/src/{configuration.namespace.title()}/Generated",
    ):
        (root / folder).mkdir(parents=True, exist_ok=True)
    rust.exports(root, descriptors, configuration=configuration)
    rust.client(root, clients, configuration=configuration, target=root / "sdk/rust/src/client/resources.rs")
    rust.host(root, context, configuration=configuration)
    # The Renderer and Native Carrier Own These Internal Names; Public SDK Faces
    # Expose Call, Client and SampleWindow Without Leaking Their Implementation Names.
    bindings = [
        "// @generated from protocol descriptors; do not edit.",
        "use crate::message::Wire;",
        "use prost_reflect::ReflectMessage;",
        "pub use crate::message::Wire as Call;",
        "pub trait Request: ReflectMessage + Default + 'static {\n    type Response: ReflectMessage + Default + 'static;\n    const ID: &'static str;\n}",
        "impl<T: Request> harmos::OperationRequest for Wire<T> { type Response = Wire<T::Response>; const ID: &'static str = T::ID; }",
    ]
    for method in context:
        request = rust.message(method["request"])
        response = rust.message(method["response"])
        identity = f"{method['package']}.{method['service']}/{method['method']}"
        bindings.append(f"impl Request for {request} {{ type Response = {response}; const ID: &'static str = {json.dumps(identity)}; }}")
    (root / "sdk/rust/src/host.rs").write_text("\n".join(bindings) + "\n", encoding="utf-8", newline="\n")

    python.exports(root / f"sdk/python/src/{configuration.namespace}/_generated", descriptors, configuration=configuration)
    python.client(
        root / f"sdk/python/src/{configuration.namespace}/_generated/client.py", clients, descriptors, configuration=configuration
    )
    python.host(root / f"sdk/python/src/{configuration.namespace}/_generated/host.py", context, descriptors, configuration=configuration)
    csharp.client(
        root,
        clients,
        configuration=configuration,
        namespace=f"{configuration.namespace}",
        target=root / f"sdk/csharp/src/{configuration.namespace.title()}/Generated/Client.g.cs",
    )
    csharp.host(
        root,
        context,
        configuration=configuration,
        namespace=configuration.namespace.title() + ".Plugins",
        target=root / f"sdk/csharp/src/{configuration.namespace.title()}/Generated/Host.g.cs",
    )
    csharp_protocol.protocols(
        source,
        descriptors=descriptor_path,
        namespace=configuration.namespace,
        tools=csharp_tools,
        destination=root / f"sdk/csharp/src/{configuration.namespace.title()}/Generated/Wire",
    )
    type_names = cpp.exports(root, descriptors, configuration=configuration)
    cpp.client(
        root,
        clients,
        configuration=configuration,
        type_names=type_names,
        namespace=f"{configuration.namespace}",
        target=root / f"sdk/cpp/include/{configuration.namespace}/_generated/client.hpp",
    )
    cpp.host(root, context, type_names, configuration=configuration)
    # Generated Public Barrel Points Hide the Wire Packages.
    shared_names = []
    for line in (root / f"sdk/python/src/{configuration.namespace}/_generated/shared.py").read_text().splitlines():
        if line.startswith("from "):
            shared_names.append(line.rsplit(" import ", 1)[-1].rsplit(" as ", 1)[-1])
    client_names = []
    for line in (root / f"sdk/python/src/{configuration.namespace}/_generated/client_types.py").read_text().splitlines():
        if line.startswith("from "):
            client_names.append(line.rsplit(" import ", 1)[-1].rsplit(" as ", 1)[-1])
    plugin_names = []
    for line in (root / f"sdk/python/src/{configuration.namespace}/_generated/plugin_types.py").read_text().splitlines():
        if line.startswith("from "):
            plugin_names.append(line.rsplit(" import ", 1)[-1].rsplit(" as ", 1)[-1])
    root_barrel = [
        "# @generated from protocol descriptors; do not edit.",
        "from harmos import Failure",
    ]
    for name in sorted(shared_names):
        root_barrel.append(f"from ._generated.shared import {name}")
    root_barrel += ["", "__all__ = " + repr(["Failure", *sorted(shared_names)])]
    (root / f"sdk/python/src/{configuration.namespace}/__init__.py").write_text(
        "\n".join(root_barrel) + "\n", encoding="utf-8", newline="\n"
    )
    client_barrel = ["# @generated from protocol descriptors; do not edit.", "from ._generated.client import ClientRoot as Client"]
    for name in sorted(client_names):
        client_barrel.append(f"from ._generated.client_types import {name}")
    client_barrel += ["", "__all__ = " + repr(["Client", *sorted(client_names)])]
    (root / f"sdk/python/src/{configuration.namespace}/client.py").write_text(
        "\n".join(client_barrel) + "\n", encoding="utf-8", newline="\n"
    )
    plugin_barrel = [
        "# @generated from protocol descriptors; do not edit.",
        "from .._plugin import Plugin",
        "from .._context import Context",
        "from .._generated.host import HostOperation",
        "from .._generated.host import HostRoot",
        "from harmos.plugin import Window as SampleWindow",
    ]
    for name in sorted(plugin_names):
        plugin_barrel.append(f"from .._generated.plugin_types import {name}")
    plugin_barrel += ["", "__all__ = " + repr(["Plugin", "Context", "HostOperation", "HostRoot", "SampleWindow", *sorted(plugin_names)])]
    (root / f"sdk/python/src/{configuration.namespace}/plugin/__init__.py").write_text(
        "\n".join(plugin_barrel) + "\n", encoding="utf-8", newline="\n"
    )
    paths = []
    for name in (
        "exports.rs",
        "host.rs",
        "client/types.rs",
        "client/resources.rs",
        "plugin/host.rs",
        "plugin/host_exports.rs",
        "plugin/types.rs",
    ):
        paths.append(root / "sdk/rust/src" / name)
    formatted = subprocess.run(
        ["rustfmt", "--edition", "2024", "--config-path", str(configuration.rustfmt or source / "rustfmt.toml"), *map(str, paths)],
        capture_output=True,
        text=True,
        cwd=source,
        creationflags=spawn_flags(),
    )
    if formatted.returncode:
        raise ValueError(
            "Generate Rust SDK: Correct the Reported Contract or Renderer Errors Before Regenerating\n"
            + formatted.stdout
            + formatted.stderr
        )
    python_paths = []
    for name in (
        "__init__.py",
        "client.py",
        "plugin/__init__.py",
        "_generated/shared.py",
        "_generated/client_types.py",
        "_generated/plugin_types.py",
        "_generated/host.py",
        "_generated/client.py",
    ):
        python_paths.append(root / f"sdk/python/src/{configuration.namespace}" / name)
    for path in python_paths:
        rendered = re.sub(r"\bimport (\w+) as \1\b", r"import \1", path.read_text(encoding="utf-8"))
        path.write_text(rendered, encoding="utf-8", newline="\n")
    subprocess.run(
        [
            sys.executable,
            "-m",
            "ruff",
            "check",
            "--config",
            str(configuration.python_style or source / "sdk/python/ruff.toml"),
            "--config",
            f'lint.isort.known-first-party=["{configuration.namespace}"]',
            "--select",
            "I,F401,RUF022",
            "--fix",
            *map(str, python_paths),
        ],
        check=True,
        cwd=source,
        creationflags=spawn_flags(),
    )
    subprocess.run(
        [
            sys.executable,
            "-m",
            "ruff",
            "format",
            "--config",
            str(configuration.python_style or source / "sdk/python/ruff.toml"),
            *map(str, python_paths),
        ],
        check=True,
        cwd=source,
        creationflags=spawn_flags(),
    )


# === Entry Point === #
