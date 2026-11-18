# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Render One Nested Contract Through Every Shared Language Backend."""

# === Imports === #

# Standard Library Imports
import ast
import asyncio
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace
from typing import Any, cast
from unittest.mock import patch

# Third Party Imports
from google.protobuf.descriptor_pb2 import FieldDescriptorProto, FileDescriptorSet
from google.protobuf.descriptor_pool import DescriptorPool
from google.protobuf.message import Message
from google.protobuf.message_factory import GetMessageClass

# Local Imports
from harmos_tools.build import csharp_protocol, python_protocol
from harmos_tools.build.resources import cpp, csharp, descriptor, python, rust, sdk
from harmos_tools.build.resources.configuration import Configuration
from harmos_tools.process import spawn_flags
from harmos_tools.source.protocol import caller_options, check_public_surfaces

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[4]

# === Types === #

# === Classes === #


class SharedGeneratorPreservesContracts(unittest.TestCase):
    def test_composed_rust_host_bindings_preserve_operation_kinds(self) -> None:
        schema = FileDescriptorSet()
        file = schema.file.add(name="example/host/v1/jobs.proto", package="example.host.v1", syntax="proto3")
        service = file.service.add(name="Jobs")
        for name, streaming in (("Get", False), ("Watch", True)):
            file.message_type.add(name=name + "Request")
            file.message_type.add(name=name + "Response")
            service.method.add(
                name=name,
                input_type=f".example.host.v1.{name}Request",
                output_type=f".example.host.v1.{name}Response",
                server_streaming=streaming,
            )
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        descriptors = root / "schema.bin"
        descriptors.write_bytes(schema.SerializeToString())
        configuration = Configuration(namespace="example", csharp_tools=root / "tools", library=FileDescriptorSet())
        with (
            patch.object(
                descriptor, "caller_options", return_value={f"example.host.v1.Jobs/{name}": "PLUGINS" for name in ("Get", "Watch")}
            ),
            patch.object(sdk.csharp_protocol, "protocols"),
            patch.object(sdk.subprocess, "run", return_value=SimpleNamespace(returncode=0)),
        ):
            sdk.generate(root=root, descriptors=descriptors, configuration=configuration)
        bindings = (root / "sdk/rust/src/host.rs").read_text()
        self.assertIn("const WATCH: bool;", bindings)
        self.assertIn("const WATCH: bool = T::WATCH;", bindings)
        for name, streaming in (("Get", "false"), ("Watch", "true")):
            self.assertIn(
                f'impl Request for example_protocol::host::{name}Request {{ type Response = example_protocol::host::{name}Response; const ID: &\'static str = "example.host.v1.Jobs/{name}"; const WATCH: bool = {streaming}; }}',
                bindings,
            )
        inventory = (root / "sdk/rust/src/plugin/host.rs").read_text()
        self.assertIn("Self::JobsGet,", inventory)
        self.assertIn("Self::JobsWatch,", inventory)

    def test_csharp_host_defaults_request_identity_before_policy_dispatch(self) -> None:
        schema = FileDescriptorSet()
        file = schema.file.add(name="example/host/v1/logs.proto", package="example.host.v1", syntax="proto3")
        file.options.csharp_namespace = "Example.Wire"
        request = file.message_type.add(name="Input")
        request.field.add(name="request_id", number=1, type=FieldDescriptorProto.TYPE_STRING)
        file.message_type.add(name="Output")
        file.service.add(name="Logs").method.add(name="Append", input_type=".example.host.v1.Input", output_type=".example.host.v1.Output")
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        with patch.object(descriptor, "caller_options", return_value={"example.host.v1.Logs/Append": "PLUGINS"}):
            methods = descriptor.table(schema)
        csharp.host(root, methods, target=root / "Host.cs", configuration=Configuration(namespace="example"))
        source = (root / "Host.cs").read_text()
        self.assertIn("string? request_id = null", source)
        self.assertIn('RequestId = request_id ?? Guid.NewGuid().ToString("N"),', source)
        self.assertNotIn("if (request_id is not null)", source)

    def test_csharp_optional_multiword_fields_keep_wire_property_case(self) -> None:
        schema = FileDescriptorSet()
        file = schema.file.add(name="example/host/v1/jobs.proto", package="example.host.v1", syntax="proto3")
        file.options.csharp_namespace = "Example.Wire"
        request = file.message_type.add(name="Input")
        for number, name in enumerate(("step_id", "maximum_seconds", "cycle_id", "execution_id"), 1):
            request.oneof_decl.add(name="_" + name)
            request.field.add(name=name, number=number, type=FieldDescriptorProto.TYPE_STRING, proto3_optional=True, oneof_index=number - 1)
        file.message_type.add(name="Output")
        file.service.add(name="Jobs").method.add(name="List", input_type=".example.host.v1.Input", output_type=".example.host.v1.Output")
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        with patch.object(descriptor, "caller_options", return_value={"example.host.v1.Jobs/List": "CLIENTS"}):
            methods = descriptor.table(schema)
        csharp.client(root, methods, target=root / "Client.cs", configuration=Configuration(namespace="example"))
        source = (root / "Client.cs").read_text()
        for property_name, parameter in (
            ("StepId", "stepId"),
            ("MaximumSeconds", "maximumSeconds"),
            ("CycleId", "cycleId"),
            ("ExecutionId", "executionId"),
        ):
            self.assertIn(f"request.{property_name} = {parameter};", source)

    def test_python_composed_payloads_import_original_wire_definitions(self) -> None:
        schema = FileDescriptorSet()
        file = schema.file.add(name="harmos/model/v1/failure.proto", package="harmos.model.v1", syntax="proto3")
        file.message_type.add(name="Failure")
        enum = file.enum_type.add(name="Classification")
        enum.value.add(name="CLASSIFICATION_UNSPECIFIED", number=0)
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        target = Path(scratch.name)
        python.exports(target, schema, configuration=Configuration(namespace="example", library=schema))
        payload = ast.parse((target / "_exports/shared/harmos/model/__init__.py").read_text())
        imports = [
            (node.module, name.name, name.asname) for node in ast.walk(payload) if isinstance(node, ast.ImportFrom) for name in node.names
        ]
        self.assertIn(("harmos._wire.model.v1.failure_pb2", "Failure", None), imports)
        shared = (target / "shared.py").read_text()
        self.assertIn("from harmos._wire.model.v1.failure_pb2 import Classification", shared)
        self.assertIn("from harmos._wire.model.v1.failure_pb2 import CLASSIFICATION_UNSPECIFIED", shared)

    def test_composed_exception_imports_its_defining_module(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        schema = FileDescriptorSet()
        schema.file.add(name="example/model/v1/values.proto", package="example.model.v1", syntax="proto3").message_type.add(name="Value")
        descriptors = root / "schema.bin"
        descriptors.write_bytes(schema.SerializeToString())
        configuration = Configuration(namespace="example", csharp_tools=root / "tools", library=FileDescriptorSet())
        with (
            patch.object(sdk.csharp_protocol, "protocols"),
            patch.object(sdk.subprocess, "run", return_value=SimpleNamespace(returncode=0)),
        ):
            sdk.generate(root=root, descriptors=descriptors, configuration=configuration)
        source = (root / "sdk/python/src/example/__init__.py").read_text()
        self.assertIn("from harmos._invocation import FailureError", source)

    def test_rust_composed_payloads_use_protocol_owner(self) -> None:
        schema = FileDescriptorSet()
        file = schema.file.add(name="harmos/model/v1/failure.proto", package="harmos.model.v1", syntax="proto3")
        file.message_type.add(name="Failure")
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        rust.exports(root, schema, configuration=Configuration(namespace="example", library=schema))
        source = (root / "sdk/rust/src/exports.rs").read_text()
        self.assertIn("pub use harmos_protocol::model::Failure;", source)
        self.assertNotIn("::harmos::model::Failure", source)

    def test_rust_client_payload_collision_uses_package_path(self) -> None:
        schema = FileDescriptorSet()
        file = schema.file.add(name="example/host/v1/storage/sources.proto", package="example.host.v1.storage", syntax="proto3")
        file.message_type.add(name="Input")
        file.message_type.add(name="StorageSources")
        file.service.add(name="Sources").method.add(
            name="Get", input_type=".example.host.v1.storage.Input", output_type=".example.host.v1.storage.StorageSources"
        )
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        with patch.object(descriptor, "caller_options", return_value={"example.host.v1.storage.Sources/Get": "CLIENTS"}):
            rust.exports(root, schema, configuration=Configuration(namespace="example"))
        source = (root / "sdk/rust/src/client/types.rs").read_text()
        self.assertIn("pub mod host {\npub mod storage {\npub use crate::wire::host::storage::StorageSources;\n}\n}", source)

    def test_rust_plugin_payload_modules_do_not_shadow_host_resources(self) -> None:
        schema = FileDescriptorSet()
        for package in ("example.host.v1.files", "example.model.v1"):
            schema.file.add(name=package.replace(".", "/") + ".proto", package=package, syntax="proto3").message_type.add(name="Stopped")
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        with patch.object(descriptor, "payload_audiences", return_value={"example.host.v1.files.Stopped": {"plugin"}}):
            rust.exports(root, schema, configuration=Configuration(namespace="example"))
        source = (root / "sdk/rust/src/plugin/types.rs").read_text()
        self.assertTrue(source.splitlines()[1].startswith("pub mod example {"))
        self.assertIn("pub mod host {", source)
        rust.host(root, [], configuration=Configuration(namespace="example"))
        self.assertIn("pub use self::host::{", (root / "sdk/rust/src/plugin/host_exports.rs").read_text())

    def test_nested_values_presence_audiences_and_call_options(self) -> None:
        # Assemble a Product Contract With the Real Caller Extension.
        library = FileDescriptorSet.FromString((ROOT / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
        pool = DescriptorPool()
        pending = {file.name: file for file in library.file}
        while pending:
            ready = [file for file in pending.values() if all(name not in pending for name in file.dependency)]
            for file in ready:
                pool.AddSerializedFile(file.SerializeToString())
                del pending[file.name]
        extension = pool.FindExtensionByName("harmos.model.v1.callers")
        assert extension.enum_type is not None
        options_type = GetMessageClass(pool.FindMessageTypeByName("google.protobuf.MethodOptions"))
        options = {}
        for audience in ("CLIENTS", "PLUGINS", "ALL"):
            declared = options_type()
            cast(Any, declared).Extensions[extension] = extension.enum_type.values_by_name[audience].number
            options[audience] = declared.SerializeToString()
        schema = FileDescriptorSet.FromString(library.SerializeToString())
        file = schema.file.add(name="example/host/v1/nested/jobs.proto", package="example.host.v1.nested", syntax="proto3")
        file.dependency.append("harmos/model/v1/callers.proto")
        file.options.csharp_namespace = "Example.Clients"
        outer = file.message_type.add(name="Envelope")
        enum = outer.enum_type.add(name="Mode")
        enum.value.add(name="MODE_UNSPECIFIED", number=0)
        enum.value.add(name="MODE_ACTIVE", number=1)
        request = outer.nested_type.add(name="Input")
        request.oneof_decl.add(name="_count")
        request.field.add(name="count", number=1, type=FieldDescriptorProto.TYPE_INT32, proto3_optional=True, oneof_index=0)
        request.field.add(name="class", number=2, type=FieldDescriptorProto.TYPE_STRING)
        request.field.add(name="type", number=3, type=FieldDescriptorProto.TYPE_INT32)
        request.field.add(
            name="modes",
            number=4,
            type=FieldDescriptorProto.TYPE_ENUM,
            type_name=".example.host.v1.nested.Envelope.Mode",
            label=FieldDescriptorProto.LABEL_REPEATED,
        )
        response = outer.nested_type.add(name="Output")
        response.field.add(name="items", number=1, type=FieldDescriptorProto.TYPE_STRING, label=FieldDescriptorProto.LABEL_REPEATED)
        service = file.service.add(name="Jobs")
        for name, audience in [("Delete", "ALL"), ("List", "CLIENTS"), ("Watch", "PLUGINS")]:
            method = service.method.add(
                name=name, input_type=".example.host.v1.nested.Envelope.Input", output_type=".example.host.v1.nested.Envelope.Output"
            )
            method.options.ParseFromString(options[audience])

        # Preserve Presence, Nested Enum Names and Independent Audiences.
        methods = [method for method in descriptor.table(schema) if method["owner"] == "example"]
        self.assertEqual(
            [(method["method"], method["face"]) for method in methods],
            [("Delete", "client"), ("Delete", "context"), ("List", "client"), ("Watch", "context")],
        )
        count, _, _, modes = methods[0]["fields"]
        self.assertTrue(count["optional"])
        self.assertTrue(count["wire_optional"])
        self.assertTrue(modes["repeated"])
        self.assertEqual(modes["csharp"], "Example.Clients.Envelope.Types.Mode")
        self.assertIn("example.host.v1.nested.Envelope.Input", descriptor.payload_audiences(schema))
        self.assertEqual(descriptor.payload_audiences(schema)["example.host.v1.nested.Envelope"], {"client", "plugin"})
        self.assertEqual(caller_options(schema)["example.host.v1.nested.Jobs/Delete"], "ALL")

        # Render All Four Languages Without Reading a Product Checkout.
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        output = Path(scratch.name)
        configuration = Configuration(namespace="example", library=library)
        clients = [method for method in methods if method["face"] == "client"]
        python.client(output / "client.py", clients, schema, configuration=configuration)
        rust.client(output, clients, target=output / "client.rs", configuration=configuration)
        csharp.client(output, clients, namespace="example", target=output / "Client.cs", configuration=configuration)
        cpp.client(output, clients, namespace="example", target=output / "client.hpp", configuration=configuration)
        parsed = ast.parse((output / "client.py").read_text())
        self.assertFalse(any(isinstance(node, ast.alias) and node.asname for node in ast.walk(parsed)))
        signature = next(node for node in ast.walk(parsed) if isinstance(node, ast.AsyncFunctionDef) and node.name == "delete")
        self.assertIn("class_", [argument.arg for argument in signature.args.args])
        self.assertIn("call_metadata", [argument.arg for argument in signature.args.kwonlyargs])
        self.assertIn("r#type", (output / "client.rs").read_text())
        self.assertIn("example_protocol::host::nested::envelope::Input", (output / "client.rs").read_text())
        self.assertIn("into_iter().map(|value| value as i32).collect()", (output / "client.rs").read_text())
        self.assertIn("@class", (output / "Client.cs").read_text())
        self.assertIn(".Modes.Add(modes)", (output / "Client.cs").read_text())
        self.assertIn("set_class_", (output / "client.hpp").read_text())

        # Generate and Check the Complete SDK Inventory Without Requiring a C# Compiler.
        source = output / "product"
        source.mkdir()
        descriptor_path = source / "descriptors.bin"
        descriptor_path.write_bytes(schema.SerializeToString())
        full = Configuration(
            namespace="example",
            library=library,
            csharp_tools=source / "compiler/tools",
            rustfmt=ROOT / "sdk/rust/rustfmt.toml",
            python_style=ROOT / "sdk/python/pyproject.toml",
        )
        projection = patch("harmos_tools.build.resources.sdk.csharp_protocol.protocols")
        compiler = projection.start()
        self.addCleanup(projection.stop)
        sdk.generate(root=source, descriptors=descriptor_path, configuration=full)
        self.assertEqual(compiler.call_args.kwargs["descriptors"], descriptor_path)
        self.assertEqual(compiler.call_args.kwargs["tools"], full.csharp_tools)
        host_source = (source / "sdk/cpp/src/generated/host.cpp").read_text()
        self.assertIn("using ::example::host::v1::nested::Envelope;", host_source)
        self.assertIn("Operation<Envelope::Input, Envelope::Output>", host_source)
        self.assertNotIn("using ::example::host::v1::nested::Envelope::Input;", host_source)
        host_source = (source / "sdk/csharp/src/Example/Generated/Host.g.cs").read_text()
        self.assertIn("request.Modes.Add(modes)", host_source)
        self.assertNotIn("Modes = modes", host_source)
        self.assertIn("request.Count = count.Value", host_source)
        self.assertIn("context.CallAsync(HostContracts.NestedJobsDelete", host_source)
        inventory = {path: (source / path).read_bytes() for path in sdk.inventory(source, full)}
        baseline_path = Path(__file__).parents[1] / "fixtures/shared_sdk_inventory.json"
        baseline = json.loads(baseline_path.read_text())
        self.assertEqual(baseline, {path.as_posix(): hashlib.sha256(content).hexdigest() for path, content in inventory.items()})

        # Separate the Python Import Package From the Protocol Namespace.
        for python_package in ("example", "example_sdk"):
            with self.subTest(python_package=python_package):
                configured = replace(full, python_package=python_package)
                destination = output / python_package
                destination.mkdir()
                sdk.generate(root=destination, descriptors=descriptor_path, configuration=configured)
                actual = {path: (destination / path).read_bytes() for path in sdk.inventory(destination, configured)}
                self.assertEqual(compiler.call_args.kwargs["namespace"], full.namespace)
                self.assertEqual(
                    {path: content for path, content in inventory.items() if path.parts[1] != "python"},
                    {path: content for path, content in actual.items() if path.parts[1] != "python"},
                )
                package_root = Path("sdk/python/src") / python_package
                python_outputs = {path: content for path, content in actual.items() if path.parts[1] == "python"}
                self.assertTrue(python_outputs)
                self.assertEqual(
                    set(python_outputs),
                    {
                        path.relative_to(destination)
                        for path in (destination / "sdk/python/src").rglob("*")
                        if path.is_file() and path.suffix in (".py", ".pyi")
                    },
                )
                self.assertEqual(
                    {package_root / path.relative_to("sdk/python/src/example") for path in inventory if path.parts[1] == "python"},
                    set(python_outputs),
                )
                for path, content in python_outputs.items():
                    self.assertTrue(path.is_relative_to(package_root))
                    original = Path("sdk/python/src/example") / path.relative_to(package_root)
                    expected_content = inventory[original]
                    for prefix in (b"from example.", b"import example.", b'__module__ = "example'):
                        expected_content = expected_content.replace(prefix, prefix.replace(b"example", python_package.encode()))
                    self.assertEqual(expected_content, content)
                    parsed_output = ast.parse(content)
                    for node in ast.walk(parsed_output):
                        if isinstance(node, ast.ImportFrom):
                            self.assertFalse((node.module or "").startswith("example.") and python_package != "example")
                        if isinstance(node, ast.Import):
                            self.assertFalse(any(name.name.startswith("example.") for name in node.names) and python_package != "example")
                    self.assertNotIn(b"example/", content)
                sdk.generate(True, root=destination, descriptors=descriptor_path, configuration=configured)
                stale_stub = destination / package_root / "_generated/obsolete.pyi"
                stale_stub.write_text("obsolete")
                self.assertRaises(ValueError, sdk.generate, True, root=destination, descriptors=descriptor_path, configuration=configured)
                sdk.generate(root=destination, descriptors=descriptor_path, configuration=configured)
                self.assertFalse(stale_stub.exists())

        sdk.generate(True, root=source, descriptors=descriptor_path, configuration=full)
        self.assertEqual(inventory, {path: (source / path).read_bytes() for path in sdk.inventory(source, full)})
        obsolete = source / "sdk/cpp/include/example/_generated/obsolete.hpp"
        obsolete.write_bytes(b"obsolete")
        self.assertRaises(ValueError, sdk.generate, True, root=source, descriptors=descriptor_path, configuration=full)
        self.assertTrue(obsolete.exists())
        sdk.generate(root=source, descriptors=descriptor_path, configuration=full)
        self.assertFalse(obsolete.exists())

        # Execute the Generated Python Call Against Real Protobuf Values.
        pool = DescriptorPool()
        pending = {file.name: file for file in schema.file}
        while pending:
            ready = [file for file in pending.values() if all(name not in pending for name in file.dependency)]
            for file in ready:
                pool.AddSerializedFile(file.SerializeToString())
                del pending[file.name]
        envelope = GetMessageClass(pool.FindMessageTypeByName("example.host.v1.nested.Envelope"))
        calls: list[tuple[Message, dict[str, object]]] = []

        async def unary(channel: object, path: str, value: Message, response: type[Message], **options: object) -> Message:
            calls.append((value, options))
            return response(items=["accepted"])

        class Connection:
            def __init__(self, channel: object) -> None:
                pass

        environment: dict[str, Any] = {
            "Envelope": envelope,
            "Channel": object,
            "CallCredentials": object,
            "Metadata": dict,
            "Sequence": list,
            "_connection": SimpleNamespace(Connection=Connection),
            "unary": unary,
        }
        parsed.body = [node for node in parsed.body if not isinstance(node, (ast.Import, ast.ImportFrom))]
        exec(compile(parsed, "generated-client", "exec"), environment)
        client = environment["Client"](object())
        result = asyncio.run(client.nested.jobs.delete("value", 7, [1], count=None, call_metadata={"token": "opaque"}, timeout=2.5))
        self.assertEqual(result, ["accepted"])
        self.assertFalse(calls[0][0].HasField("count"))
        self.assertEqual(getattr(calls[0][0], "class"), "value")
        self.assertEqual(list(cast(Any, calls[0][0]).modes), [1])
        self.assertEqual(calls[0][1]["metadata"], {"token": "opaque"})
        self.assertEqual(calls[0][1]["timeout"], 2.5)
        asyncio.run(client.nested.jobs.delete("value", 7, [1], count=0))
        self.assertTrue(calls[1][0].HasField("count"))
        self.assertEqual(cast(Any, calls[1][0]).count, 0)

    def test_product_payload_names_and_transport_options_remain_independent(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        output = Path(scratch.name)
        for namespace in ("example", "harmos"):
            for field_names in (("metadata",), ("call_metadata", "metadata"), ("timeout", "credentials", "call_timeout")):
                with self.subTest(namespace=namespace, fields=field_names):
                    schema = FileDescriptorSet()
                    file = schema.file.add(name=f"{namespace}/v1/jobs.proto", package=f"{namespace}.v1", syntax="proto3")
                    file.options.csharp_namespace = namespace.title() + ".Clients"
                    request = file.message_type.add(name="Input")
                    for number, name in enumerate(field_names, 1):
                        request.field.add(name=name, number=number, type=FieldDescriptorProto.TYPE_STRING)
                    response = file.message_type.add(name="CapturesStopResponse")
                    response.field.add(name="value", number=1, type=FieldDescriptorProto.TYPE_STRING)
                    service = file.service.add(name="Jobs")
                    service.method.add(
                        name="Stop", input_type=f".{file.package}.Input", output_type=f".{file.package}.CapturesStopResponse"
                    )
                    configuration = Configuration(namespace=namespace, library=FileDescriptorSet())
                    methods = descriptor.table(schema)
                    process_patch = patch.object(python.subprocess, "run")
                    process_patch.start()
                    try:
                        python.client(output / "client.py", methods, schema, configuration=configuration)
                    finally:
                        process_patch.stop()
                    csharp.client(output, methods, namespace=namespace, target=output / "Client.cs", configuration=configuration)
                    self.assertIn(f"global::{namespace.title()}.Clients.CapturesStopResponse", (output / "Client.cs").read_text())
                    parsed = ast.parse((output / "client.py").read_text())
                    signature = next(node for node in ast.walk(parsed) if isinstance(node, ast.AsyncFunctionDef))
                    self.assertEqual([argument.arg for argument in signature.args.args][1:], list(field_names))
                    transport = [argument.arg for argument in signature.args.kwonlyargs]
                    self.assertTrue(set(transport).isdisjoint(field_names))
                    pool = DescriptorPool()
                    pool.Add(file)
                    input_type = GetMessageClass(pool.FindMessageTypeByName(file.package + ".Input"))
                    response_type = GetMessageClass(pool.FindMessageTypeByName(file.package + ".CapturesStopResponse"))
                    calls: list[tuple[Message, dict[str, object]]] = []

                    async def unary(
                        channel: object,
                        path: str,
                        value: Message,
                        response: type[Message],
                        *,
                        captured: list[tuple[Message, dict[str, object]]] = calls,
                        **options: object,
                    ) -> Message:
                        captured.append((value, options))
                        return response(value="accepted")

                    class Connection:
                        def __init__(self, channel: object) -> None:
                            pass

                    environment: dict[str, Any] = {
                        "Input": input_type,
                        "CapturesStopResponse": response_type,
                        "Channel": object,
                        "CallCredentials": object,
                        "Metadata": dict,
                        "_connection": SimpleNamespace(Connection=Connection),
                        "unary": unary,
                    }
                    parsed.body = [node for node in parsed.body if not isinstance(node, (ast.Import, ast.ImportFrom))]
                    exec(compile(parsed, "product-client", "exec"), environment)
                    client = environment["Client"](object())
                    options = dict(zip(transport, ({"token": "transport"}, 2.5, None), strict=True))
                    result = asyncio.run(client.jobs.stop(*field_names, **options))
                    self.assertEqual(result, "accepted")
                    self.assertEqual([getattr(calls[0][0], name) for name in field_names], list(field_names))
                    self.assertEqual(calls[0][1]["metadata"], {"token": "transport"})
                    self.assertEqual(calls[0][1]["timeout"], 2.5)

    def test_small_product_descriptors_generate_without_aliases_or_composition(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        for namespace, python_package in (("example", None), ("another", None), ("example", "example_sdk")):
            with self.subTest(namespace=namespace, python_package=python_package):
                schema = FileDescriptorSet()
                for package in (namespace + ".model.v1", namespace + ".model.v1.nested", "harmos.model.v1"):
                    file = schema.file.add(name=package.replace(".", "/") + "/values.proto", package=package, syntax="proto3")
                    for name in ("Call", "Client", "Window", "Failure", "Value"):
                        file.message_type.add(name=name)
                    mode = file.enum_type.add(name="Mode")
                    mode.value.add(name="MODE_UNSPECIFIED", number=0)
                package = python_package or namespace
                source = root / package
                source.mkdir()
                descriptor_path = source / "descriptors.bin"
                descriptor_path.write_bytes(schema.SerializeToString())
                configuration = Configuration(
                    namespace=namespace, python_package=python_package, library=FileDescriptorSet(), csharp_tools=source / "tools"
                )
                protocol_patch = patch.object(sdk.csharp_protocol, "protocols")
                process_patch = patch.object(sdk.subprocess, "run", return_value=SimpleNamespace(returncode=0))
                protocol_patch.start()
                process_patch.start()
                try:
                    sdk.generate(root=source, descriptors=descriptor_path, configuration=configuration)
                    sdk.generate(True, root=source, descriptors=descriptor_path, configuration=configuration)
                finally:
                    process_patch.stop()
                    protocol_patch.stop()
                check_public_surfaces(source, namespace=namespace, python_package=Path(f"sdk/python/src/{package}"))
                inventory = sdk.inventory(source, configuration)
                self.assertTrue(inventory)
                for relative in inventory:
                    text = (source / relative).read_text()
                    if relative.suffix == ".py":
                        parsed = ast.parse(text)
                        self.assertFalse(any(isinstance(node, ast.alias) and node.asname for node in ast.walk(parsed)), str(relative))
                    if relative.suffix == ".rs":
                        self.assertNotRegex(text, r"(?:pub )?use [^;]+ as ")
                    if relative.suffix == ".hpp":
                        self.assertNotRegex(text, r"using \w+ = ")
                public = (source / f"sdk/cpp/include/{namespace}/messages.hpp").read_text()
                self.assertIn(f"using ::{namespace}::detail::payloads::model::Client;", public)
                self.assertNotIn("using ::harmos::detail::", public)
                self.assertIn(
                    "from . import nested",
                    (source / f"sdk/python/src/{package}/_generated/_exports/shared/model/__init__.py").read_text(),
                )
                self.assertIn("pub mod nested", (source / "sdk/rust/src/exports.rs").read_text())

    def test_python_package_rejects_invalid_identifiers(self) -> None:
        for python_package in ("", "example-sdk", "example.sdk", "123example", "class", " example", "../example", "example/sdk"):
            with self.subTest(python_package=python_package):
                self.assertRaisesRegex(ValueError, "Set Configuration.python_package", Configuration, python_package=python_package)

    def test_product_wire_compilation_uses_configured_inputs_and_private_modules(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        schema = FileDescriptorSet()
        file = schema.file.add(name="example/model/v1/values.proto", package="example.model.v1", syntax="proto3")
        file.message_type.add(name="Value")
        descriptor_path = root / "schema.bin"
        descriptor_path.write_bytes(schema.SerializeToString())
        proto_root = root / "contracts"
        proto_file = proto_root / file.name
        proto_file.parent.mkdir(parents=True)
        proto_file.write_text('syntax = "proto3";')
        self.assertEqual(python_protocol.protocols(root=root, namespace="example", proto_root=proto_root), (file.name,))
        commands = []

        def compile_protocol(command: list[str], **options: object) -> SimpleNamespace:
            if "grpc_tools.protoc" not in command:
                return SimpleNamespace(returncode=0)
            commands.append(command)
            self.assertEqual(options["cwd"], root)
            output = Path(next(argument.removeprefix("--python_out=") for argument in command if argument.startswith("--python_out=")))
            for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py"):
                path = output / (file.name.removesuffix(".proto") + suffix)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(
                    "from example.model.v1 import values_pb2\n"
                    "import example.model.v1.values_pb2\n"
                    "from example import model\n"
                    "from shared.model.v1 import shared_pb2\n"
                    "BuildTopDescriptorsAndMessages(DESCRIPTOR, 'example.model.v1.values_pb2', _globals)\n"
                )
            return SimpleNamespace(returncode=0)

        for python_package in (None, "example", "example_sdk"):
            with self.subTest(python_package=python_package):
                package = python_package or "example"
                output = root / ("bindings_" + str(python_package))
                stale = output / package / "_wire/obsolete_pb2.py"
                stale.parent.mkdir(parents=True)
                stale.write_text("obsolete")
                commands.clear()
                configuration = Configuration(namespace="example", python_package=python_package)
                with patch.object(python_protocol.subprocess, "run", side_effect=compile_protocol):
                    python_protocol.generate(
                        output,
                        root=root,
                        proto_root=proto_root,
                        descriptors=descriptor_path,
                        configuration=configuration,
                        module_prefixes={"shared": "shared._wire"},
                    )
                self.assertEqual(len(commands), 1)
                self.assertIn(f"-I{proto_root}", commands[0])
                self.assertEqual(commands[0][-1], file.name)
                self.assertFalse(stale.exists())
                self.assertFalse((output / file.name.removesuffix(".proto")).with_name("values_pb2.py").exists())
                for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py"):
                    generated = output / package / ("_wire/model/v1/values" + suffix)
                    source = generated.read_text()
                    self.assertIn(f"from {package}._wire.model.v1 import values_pb2", source)
                    self.assertIn(f"import {package}._wire.model.v1.values_pb2", source)
                    self.assertIn(f"from {package}._wire import model", source)
                    self.assertIn("from shared._wire.model.v1 import shared_pb2", source)
                    self.assertIn(f"BuildTopDescriptorsAndMessages(DESCRIPTOR, '{package}._wire.model.v1.values_pb2'", source)
                shared = (output / package / "_generated/shared.py").read_text()
                self.assertIn(f"from {package}._wire.model.v1.values_pb2 import Value", shared)
                self.assertIn(f'Value.__module__ = "{package}"', shared)
                self.assertTrue((output / package / "_generated/host.py").is_file())

        # Import Real Compiler Output and Round-Trip Its Public Message.
        proto_file.write_text('syntax = "proto3";\npackage example.model.v1;\nmessage Value {}\n')
        for python_package in (None, "example_sdk"):
            with self.subTest(real_compiler=True, python_package=python_package):
                package = python_package or "example"
                output = root / ("compiled_" + package)
                python_protocol.generate(
                    output,
                    root=root,
                    proto_root=proto_root,
                    descriptors=descriptor_path,
                    configuration=Configuration(namespace="example", python_package=python_package),
                )
                command = [
                    sys.executable,
                    "-c",
                    f"from {package}._generated.shared import Value\n"
                    "assert Value.DESCRIPTOR.full_name == 'example.model.v1.Value'\n"
                    f"assert Value.__module__ == '{package}'\n"
                    "assert Value.FromString(Value().SerializeToString()) == Value()",
                ]
                subprocess.run(command, check=True, cwd=output, creationflags=spawn_flags())
        file.options.csharp_namespace = "Example.Wire"
        source = "namespace Example.Wire {\npublic sealed partial class Value : global::Example.Wire.Base {}\n}"
        rendered = csharp_protocol._wire_source(source, "Values.cs", file, set(), None, {"Example.Wire": "ExampleSDK.Models"})
        self.assertIn("namespace ExampleSDK.Models;", rendered)
        self.assertIn("global::ExampleSDK.Models.Base", rendered)

    def test_clashing_message_names_remain_qualified_in_every_language(self) -> None:
        schema = FileDescriptorSet()
        for parent in ("first", "second"):
            package = "example.host.v1." + parent
            file = schema.file.add(name=f"example/v1/{parent}.proto", package=package, syntax="proto3")
            file.options.csharp_namespace = "Example." + parent.title()
            context = file.message_type.add(name="Context")
            context.field.add(name="value", number=1, type=FieldDescriptorProto.TYPE_STRING)
            request = file.message_type.add(name="Input")
            request.field.add(name="context", number=1, type=FieldDescriptorProto.TYPE_MESSAGE, type_name=f".{package}.Context")
            file.message_type.add(name="Client")
            service = file.service.add(name="Jobs")
            service.method.add(name="Run", input_type=f".{package}.Input", output_type=f".{package}.Client")
        configuration = Configuration(namespace="example", library=FileDescriptorSet())
        clients = descriptor.table(schema)
        hosts = [descriptor.Method(**{**method, "face": "context"}) for method in clients]
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        with patch.object(sdk.subprocess, "run", return_value=SimpleNamespace(returncode=0)):
            types = cpp.exports(root, schema, configuration=configuration)
            cpp.client(root, clients, type_names=types, namespace="example", target=root / "client.hpp", configuration=configuration)
            cpp.host(root, hosts, type_names=types, configuration=configuration)
            python.client(root / "client.py", clients, schema, configuration=configuration)
            python.host(root / "host.py", hosts, schema, configuration=configuration)
            rust.client(root, clients, target=root / "client.rs", configuration=configuration)
            rust.host(root, hosts, target=root / "host.rs", configuration=configuration)
            csharp.client(root, clients, target=root / "Client.cs", configuration=configuration)
            csharp.host(root, hosts, target=root / "Host.cs", configuration=configuration)
        for parent in ("first", "second"):
            for filename in ("client.py", "host.py"):
                text = (root / filename).read_text()
                self.assertIn(f"example._wire.v1.{parent}_pb2.Context", text)
                ast.parse(text)
            for filename in ("client.rs", "host.rs"):
                self.assertIn(f"example_protocol::host::{parent}::Context", (root / filename).read_text())
            for filename in ("Client.cs", "Host.cs"):
                self.assertIn(f"global::Example.{parent.title()}.Context", (root / filename).read_text())
            text = (root / "sdk/cpp/src/generated/host.cpp").read_text()
            self.assertIn(f"::example::host::v1::{parent}::Input", text)
            self.assertNotIn(f"using ::example::host::v1::{parent}::Input;", text)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
