# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Generate the Operations Reference from the Protocol Definitions and Their Comments"""

# === Imports === #

# Standard Library Imports
import re
import shutil
import tempfile
from pathlib import Path

# isort: split

# Third Party Imports
from google.protobuf.descriptor_pb2 import (
    DescriptorProto,
    EnumDescriptorProto,
    FieldDescriptorProto,
    FileDescriptorProto,
    FileDescriptorSet,
)

# isort: split

# Local Imports
from ..release import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

OUTPUT = "target/docs/generated/reference/operations"
WELL_KNOWN = "https://protobuf.dev/reference/protobuf/google.protobuf/"
SCALARS = {
    FieldDescriptorProto.TYPE_DOUBLE: "double",
    FieldDescriptorProto.TYPE_FLOAT: "float",
    FieldDescriptorProto.TYPE_INT64: "int64",
    FieldDescriptorProto.TYPE_UINT64: "uint64",
    FieldDescriptorProto.TYPE_INT32: "int32",
    FieldDescriptorProto.TYPE_FIXED64: "fixed64",
    FieldDescriptorProto.TYPE_FIXED32: "fixed32",
    FieldDescriptorProto.TYPE_BOOL: "bool",
    FieldDescriptorProto.TYPE_STRING: "string",
    FieldDescriptorProto.TYPE_BYTES: "bytes",
    FieldDescriptorProto.TYPE_UINT32: "uint32",
    FieldDescriptorProto.TYPE_SFIXED32: "sfixed32",
    FieldDescriptorProto.TYPE_SFIXED64: "sfixed64",
    FieldDescriptorProto.TYPE_SINT32: "sint32",
    FieldDescriptorProto.TYPE_SINT64: "sint64",
}
# Source Code Info Path Components (descriptor.proto Field Numbers).
MESSAGES, ENUMS, SERVICES = 4, 5, 6
FIELDS, NESTED_MESSAGES, NESTED_ENUMS = 2, 3, 4
METHODS = VALUES = 2

# === Types === #

# === Classes === #

# === Public Functions === #


def generate(
    directory: Path,
    *,
    descriptors: Path | FileDescriptorSet | None = None,
    output: Path = Path(OUTPUT),
    namespaces: tuple[str, ...] = (),
    exclude: tuple[str, ...] = ("google.", "harmos.host."),
    namespace: str = "harmos",
    overview_paragraphs: tuple[str, ...] | None = None,
) -> Path:
    """Write One Page per Protocol Package and an Overview Under target/docs/generated/."""
    if descriptors is None:
        scratch_directory = tempfile.TemporaryDirectory()
        try:
            image = Path(scratch_directory.name) / "protocol.binpb"
            policy.execute(directory, ["buf", "build", "--as-file-descriptor-set", "-o", str(image)])
            descriptors = FileDescriptorSet.FromString(image.read_bytes())
        finally:
            scratch_directory.cleanup()
    elif isinstance(descriptors, Path):
        descriptors = FileDescriptorSet.FromString((directory / descriptors).read_bytes())
    packages: dict[str, list[FileDescriptorProto]] = {}
    for file in sorted(descriptors.file, key=lambda item: item.name):
        if (
            file.package.startswith(exclude)
            or namespaces
            and not any(file.package == prefix or file.package.startswith(prefix + ".") for prefix in namespaces)
        ):
            continue
        packages.setdefault(file.package, []).append(file)
    owners = {}
    for package, files in packages.items():
        for file in files:
            for name, _, _ in _definitions(file):
                owners[f".{package}.{name}"] = (package, name)
    output = policy.inside(directory, str(output))
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    ordered = sorted(packages, key=lambda package: (not package.startswith(namespace + "."), package))
    for order, package in enumerate(ordered, start=1):
        content = page(package, packages[package], owners, order, namespace=namespace)
        (output / f"{package}.md").write_text(content, encoding="utf-8", newline="\n")
    (output / "overview.md").write_text(overview(ordered, packages, paragraphs=overview_paragraphs), encoding="utf-8", newline="\n")
    return output


def page(package: str, files: list[FileDescriptorProto], owners: dict, order: int, *, namespace: str = "harmos") -> str:
    """One Package: Its Services with Their Operations, Then Its Messages and Enums."""
    lines = _frontmatter(package, f"Operations and messages of the {package} protocol package.", order)
    services = [(file, index, service) for file in files for index, service in enumerate(file.service)]
    if services:
        lines += ["## Services", ""]
    for file, index, service in sorted(services, key=lambda item: item[2].name):
        comments = _comments(file)
        lines += [f"### {service.name} Service", "", *_paragraph(comments.get((SERVICES, index)))]
        lines += ["| Operation | Request | Response | Callers | Description |", "| --- | --- | --- | --- | --- |"]
        for position, method in enumerate(service.method):
            request = ("stream " if method.client_streaming else "") + _type(method.input_type, package, owners)
            response = ("stream " if method.server_streaming else "") + _type(method.output_type, package, owners)
            audience = "Host" if package.startswith(namespace + ".plugin.") else "Product-Defined"
            description = _cell(comments.get((SERVICES, index, METHODS, position)))
            lines.append(f"| `{service.name}.{method.name}` | {request} | {response} | {audience} | {description} |")
        lines.append("")
    definitions = [(file, name, definition, path) for file in files for name, definition, path in _definitions(file)]
    messages: list[tuple[FileDescriptorProto, str, DescriptorProto, tuple[int, ...]]] = []
    enums: list[tuple[FileDescriptorProto, str, EnumDescriptorProto, tuple[int, ...]]] = []
    for file, name, definition, path in definitions:
        if isinstance(definition, EnumDescriptorProto):
            enums.append((file, name, definition, path))
        elif not definition.options.map_entry:
            messages.append((file, name, definition, path))
    if messages:
        lines += ["## Messages", ""]
    for file, name, message, path in sorted(messages, key=lambda item: item[1]):
        comments = _comments(file)
        lines += [f"### {name}", "", *_paragraph(comments.get(path))]
        if not message.field:
            lines += ["This message has no fields.", ""]
            continue
        lines += ["| Field | Number | Type | Description |", "| --- | --- | --- | --- |"]
        for position, field in enumerate(message.field):
            kind = _field_type(field, package, owners, definitions)
            description = _cell(comments.get((*path, FIELDS, position)))
            if field.HasField("oneof_index") and not field.proto3_optional:
                choice = f"One of `{message.oneof_decl[field.oneof_index].name}`."
                description = f"{choice} {description}".strip()
            lines.append(f"| `{field.name}` | {field.number} | {kind} | {description} |")
        lines.append("")
    if enums:
        lines += ["## Enums", ""]
    for file, name, enum, path in sorted(enums, key=lambda item: item[1]):
        comments = _comments(file)
        lines += [f"### {name}", "", *_paragraph(comments.get(path))]
        lines += ["| Value | Number | Description |", "| --- | --- | --- |"]
        for position, value in enumerate(enum.value):
            lines.append(f"| `{value.name}` | {value.number} | {_cell(comments.get((*path, VALUES, position)))} |")
        lines.append("")
    headings = [_anchor(line.lstrip("#").strip()) for line in lines if line.startswith("##")]
    duplicates = sorted({anchor for anchor in headings if headings.count(anchor) > 1})
    if duplicates:
        raise policy.ToolError(f"Operations Reference: {package} Has Colliding Headings: {', '.join(duplicates)}")
    return "\n".join(lines).rstrip() + "\n"


def overview(ordered: list[str], packages: dict[str, list[FileDescriptorProto]], *, paragraphs: tuple[str, ...] | None = None) -> str:
    lines = _frontmatter("Overview", "Protocol packages, their services and their operations.", 0)
    lines += (
        list(paragraphs)
        if paragraphs is not None
        else [
            "Each page lists one protocol package, its messages and enums, and its declared services.",
            "The plugin protocol defines facilities the host invokes on a plugin; shared model messages",
            "describe their payloads. Schematic packages demonstrate product-owned remote wrappers.",
            "",
            "Host operations are native-only and products choose their own remote wrappers. See",
            "[Native Storage Operations](../../../storage/) and [Architecture](../../../architecture/)",
            "for native contracts. The retired host-client projection is excluded from this reference.",
        ]
    )
    lines += [
        "",
        "| Package | Services | Operations | Messages |",
        "| --- | ---: | ---: | ---: |",
    ]
    for package in ordered:
        files = packages[package]
        services = sum(len(file.service) for file in files)
        operations = sum(len(service.method) for file in files for service in file.service)
        messages = sum(
            isinstance(item[1], DescriptorProto) and not item[1].options.map_entry for file in files for item in _definitions(file)
        )
        lines.append(f"| [{package}](../{package}/) | {services} | {operations} | {messages} |")
    return "\n".join(lines) + "\n"


# === Private Functions === #


def _frontmatter(title: str, description: str, order: int) -> list[str]:
    return ["---", f"title: {title}", f"description: {description}", "sidebar:", f"  order: {order}", "---", ""]


def _definitions(file: FileDescriptorProto) -> list[tuple[str, DescriptorProto | EnumDescriptorProto, tuple[int, ...]]]:
    """Every Message and Enum of a File with Its Package-Relative Name and Source Path."""
    result: list[tuple[str, DescriptorProto | EnumDescriptorProto, tuple[int, ...]]] = []

    def visit(message: DescriptorProto, name: str, path: tuple[int, ...]) -> None:
        result.append((name, message, path))
        for index, nested in enumerate(message.nested_type):
            visit(nested, f"{name}.{nested.name}", (*path, NESTED_MESSAGES, index))
        for index, enum in enumerate(message.enum_type):
            result.append((f"{name}.{enum.name}", enum, (*path, NESTED_ENUMS, index)))

    for index, message in enumerate(file.message_type):
        visit(message, message.name, (MESSAGES, index))
    for index, enum in enumerate(file.enum_type):
        result.append((enum.name, enum, (ENUMS, index)))
    return result


def _comments(file: FileDescriptorProto) -> dict[tuple[int, ...], str]:
    return {
        tuple(location.path): (location.leading_comments or location.trailing_comments).strip()
        for location in file.source_code_info.location
        if location.leading_comments or location.trailing_comments
    }


def _type(name: str, package: str, owners: dict) -> str:
    if name.startswith(".google.protobuf."):
        short = name.removeprefix(".google.protobuf.")
        return f"[google.protobuf.{short}]({WELL_KNOWN}#{short.lower()})"
    if name not in owners:
        return f"`{name.lstrip('.')}`"
    owner, relative = owners[name]
    if owner == package:
        return f"[{relative}](#{_anchor(relative)})"
    return f"[{owner}.{relative}](../{owner}/#{_anchor(relative)})"


def _field_type(field: FieldDescriptorProto, package: str, owners: dict, definitions: list) -> str:
    if field.type in SCALARS:
        kind = SCALARS[field.type]
    else:
        entry = next(
            (item[2] for item in definitions if f".{package}.{item[1]}" == field.type_name and isinstance(item[2], DescriptorProto)),
            None,
        )
        if entry is not None and entry.options.map_entry:
            key, value = entry.field
            return f"map<{_field_type(key, package, owners, definitions)}, {_field_type(value, package, owners, definitions)}>"
        kind = _type(field.type_name, package, owners)
    if field.label == FieldDescriptorProto.LABEL_REPEATED:
        return f"repeated {kind}"
    return f"optional {kind}" if field.proto3_optional else kind


def _escape(text: str) -> str:
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def _paragraph(text: str | None) -> list[str]:
    if not text:
        return []
    lines = [re.sub(r"^(\s*)([#>])", r"\1\\\2", _escape(line.strip())) for line in text.splitlines()]
    return [*lines, ""]


def _cell(text: str | None) -> str:
    return " ".join(_escape(line.strip()) for line in (text or "").splitlines()).replace("|", "\\|")


def _anchor(heading: str) -> str:
    """The Heading Identifier the Site Assigns, Following GitHub Slugging."""
    return re.sub(r"[^\w\- ]", "", heading.lower()).replace(" ", "-")


# === Entry Point === #

if __name__ == "__main__":
    print(generate(policy.root()))
