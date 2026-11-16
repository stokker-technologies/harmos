# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Interval Rows as Packed Columns, Split Into Window Parts."""

# === Imports === #

# Standard Library Imports
from bisect import bisect_right
from collections.abc import Sequence
from itertools import accumulate
from typing import cast

# Third Party Imports
from google.protobuf import message_factory
from google.protobuf.descriptor import Descriptor, FieldDescriptor
from google.protobuf.descriptor_pb2 import EDITION_2023, FieldDescriptorProto, FileDescriptorProto
from google.protobuf.message import Message

from harmos._wire.plugin.v1.stream_pb2 import SampleBatch

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# Upper Bound of InstanceSamplesResponse.batch Framing Around the Columns Bytes:
# response tag and length (1 + 5), offset and count (2 * (1 + 10)), columns tag and length (1 + 5), last (2).
FRAMING_BYTES = 36

# Upper Bound of a Packed Column's Length Prefix.
LENGTH_BYTES = 5

FIXED_BYTES = {
    FieldDescriptor.TYPE_DOUBLE: 8,
    FieldDescriptor.TYPE_FIXED64: 8,
    FieldDescriptor.TYPE_SFIXED64: 8,
    FieldDescriptor.TYPE_FLOAT: 4,
    FieldDescriptor.TYPE_FIXED32: 4,
    FieldDescriptor.TYPE_SFIXED32: 4,
    FieldDescriptor.TYPE_BOOL: 1,
}
SIGNED = (FieldDescriptor.TYPE_INT32, FieldDescriptor.TYPE_INT64, FieldDescriptor.TYPE_ENUM)
ZIGZAG = (FieldDescriptor.TYPE_SINT32, FieldDescriptor.TYPE_SINT64)
UNSIGNED = (FieldDescriptor.TYPE_UINT32, FieldDescriptor.TYPE_UINT64)
DELIMITED = (FieldDescriptor.TYPE_STRING, FieldDescriptor.TYPE_BYTES, FieldDescriptor.TYPE_MESSAGE)

# === Types === #

type Columns = list[list[object]]

# === Classes === #


class OversizedSampleError(ValueError):
    """One Sample Exceeds the Message Limit, so No Part Can Carry It."""


# === Public Functions === #


def columns_type(row: type[Message], stream: str) -> type[Message]:
    """Refuse Rows Without One Value per Field and Derive Their Columns Message."""
    descriptor = cast(Descriptor, row.DESCRIPTOR)
    for field in descriptor.fields:
        if field.is_repeated or field.containing_oneof is not None or field.type == FieldDescriptor.TYPE_GROUP:
            raise ValueError(
                f"Declare Observer {stream}: Row {descriptor.full_name} Field {field.name} Is Repeated, a Map, a Group or in a Oneof;"
                " give every interval row field exactly one plain value"
            )
    name = f"harmos.columns.{descriptor.full_name}.Columns"
    pool = descriptor.file.pool
    try:
        return message_factory.GetMessageClass(pool.FindMessageTypeByName(name))
    except KeyError:
        pass
    imports = {descriptor.file.name}
    for field in descriptor.fields:
        if field.message_type is not None:
            imports.add(field.message_type.file.name)
        elif field.enum_type is not None:
            imports.add(field.enum_type.file.name)
    file = FileDescriptorProto(
        name=f"harmos/columns/{descriptor.full_name}.proto",
        package=name.removesuffix(".Columns"),
        dependency=sorted(imports),
        syntax="editions",
        edition=EDITION_2023,
    )
    columns = file.message_type.add(name="Columns")
    for field in descriptor.fields:
        entry = columns.field.add(name=field.name, number=field.number, label=FieldDescriptorProto.LABEL_REPEATED, type=field.type)
        if field.message_type is not None:
            entry.type_name = "." + field.message_type.full_name
        elif field.enum_type is not None:
            entry.type_name = "." + field.enum_type.full_name
    pool.AddSerializedFile(file.SerializeToString())
    return message_factory.GetMessageClass(pool.FindMessageTypeByName(name))


def check_columns(row: type[Message], columns: type[Message], stream: str) -> None:
    """Require the Author's Columns to Repeat Every Row Field by Number and Type."""
    expected = {field.number: field for field in cast(Descriptor, row.DESCRIPTOR).fields}
    actual = {field.number: field for field in cast(Descriptor, columns.DESCRIPTOR).fields}
    name = columns.DESCRIPTOR.full_name
    for number in sorted(expected.keys() | actual.keys()):
        field, column = expected.get(number), actual.get(number)
        if field is None or column is None:
            raise ValueError(f"Declare Window Observer {stream}: Columns {name} and Row Differ at Field {number}; mirror every row field")
        if (
            not column.is_repeated
            or column.type != field.type
            or _type_name(column) != _type_name(field)
            or column.message_type is not None
            and column.message_type.GetOptions().map_entry
        ):
            raise ValueError(
                f"Declare Window Observer {stream}: Columns Field {column.name} Must Repeat Row Field {field.name} With Its Type"
            )


def append(columns: Columns, row: Message) -> None:
    """Add One Sample's Row to Its Columns, Copying Values So Later Row Changes Cannot Leak."""
    for values, field in zip(columns, cast(Descriptor, row.DESCRIPTOR).fields, strict=True):
        value = getattr(row, field.name)
        if field.message_type is not None:
            copy = type(value)()
            copy.CopyFrom(value)
            value = copy
        values.append(value)


def extract(row: type[Message], columns: Message, count: int) -> Columns:
    """Read the Author's Columns by Row Field Number, One Value per Sample."""
    names = {field.number: field.name for field in cast(Descriptor, columns.DESCRIPTOR).fields}
    result: Columns = []
    for field in cast(Descriptor, row.DESCRIPTOR).fields:
        values = list(getattr(columns, names[field.number]))
        if len(values) != count:
            raise ValueError(
                f"Answer Samples Window: Column {names[field.number]} Holds {len(values)} Values for {count} Samples;"
                " return one value per sample in every column"
            )
        result.append(values)
    return result


def batches(row: type[Message], columns: Columns, count: int, limit: int) -> list[SampleBatch]:
    """Split Columns at Sample Boundaries So Each Framed Part Fits the Message Limit."""
    fields = cast(Descriptor, row.DESCRIPTOR).fields
    kind = columns_type(row, row.DESCRIPTOR.full_name)
    whole = _encode(kind, fields, columns, 0, count)
    if FRAMING_BYTES + len(whole) <= limit:
        return [SampleBatch(offset=0, count=count, columns=whole, last=True)]
    overhead = FRAMING_BYTES + sum(_varint_bytes(field.number << 3) + LENGTH_BYTES for field in fields if field.type not in DELIMITED)
    totals = [0] * count
    for field, values in zip(fields, columns, strict=True):
        totals = [total + size for total, size in zip(totals, _sizes(field, values), strict=True)]
    prefix = [0, *accumulate(totals)]
    parts: list[SampleBatch] = []
    start = 0
    while start < count:
        end = bisect_right(prefix, prefix[start] + limit - overhead) - 1
        if end <= start:
            raise OversizedSampleError(
                f"Split Samples Window: One {row.DESCRIPTOR.full_name} Sample Needs {overhead + totals[start]} Bytes,"
                f" Beyond the {limit}-Byte Message Limit; reduce the row size or raise MessageSettings.message_bytes"
            )
        parts.append(SampleBatch(offset=start, count=end - start, columns=_encode(kind, fields, columns, start, end), last=end == count))
        start = end
    return parts


# === Private Functions === #


def _type_name(field: FieldDescriptor) -> str:
    if field.message_type is not None:
        return field.message_type.full_name
    if field.enum_type is not None:
        return field.enum_type.full_name
    return ""


def _encode(kind: type[Message], fields: Sequence[FieldDescriptor], columns: Columns, start: int, end: int) -> bytes:
    message = kind()
    for field, values in zip(fields, columns, strict=True):
        getattr(message, field.name).extend(values[start:end])
    return message.SerializeToString()


def _varint_bytes(value: int) -> int:
    return max(1, (value.bit_length() + 6) // 7)


def _sizes(field: FieldDescriptor, values: list[object]) -> list[int]:
    """Encoded Bytes of Each Sample's Value Within Its Column."""
    fixed = FIXED_BYTES.get(field.type)
    if fixed is not None:
        return [fixed] * len(values)
    if field.type in SIGNED:
        return [10 if value < 0 else _varint_bytes(value) for value in cast(list[int], values)]
    if field.type in ZIGZAG:
        return [_varint_bytes(2 * value if value >= 0 else -2 * value - 1) for value in cast(list[int], values)]
    if field.type in UNSIGNED:
        return [_varint_bytes(value) for value in cast(list[int], values)]
    tag = _varint_bytes(field.number << 3)
    if field.type == FieldDescriptor.TYPE_STRING:
        lengths = [len(value.encode()) for value in cast(list[str], values)]
    elif field.type == FieldDescriptor.TYPE_BYTES:
        lengths = [len(value) for value in cast(list[bytes], values)]
    else:
        lengths = [value.ByteSize() for value in cast(list[Message], values)]
    return [tag + _varint_bytes(length) + length for length in lengths]


# === Entry Point === #
