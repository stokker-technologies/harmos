# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Interval Windows Are Answered as Packed Columns Split Within the Message Limit."""

# === Imports === #

# Standard Library Imports
from unittest import IsolatedAsyncioTestCase

# Third Party Imports
import grpc
from google.protobuf import duration_pb2, message_factory, struct_pb2
from google.protobuf.descriptor_pb2 import FieldDescriptorProto, FileDescriptorProto
from google.protobuf.descriptor_pool import DescriptorPool
from google.protobuf.duration_pb2 import Duration
from google.protobuf.message import Message
from google.protobuf.struct_pb2 import Struct
from google.protobuf.timestamp_pb2 import Timestamp
from grpc import StatusCode
from grpc.aio import AioRpcError

from harmos import MessageSettings
from harmos._columns import Columns, append, batches, columns_type, extract
from harmos._generated.proto.plugin.v1.instance_pb2 import InstanceGetRequest, InstanceSamplesRequest, InstanceSamplesResponse
from harmos._generated.proto.plugin.v1.instance_pb2_grpc import InstanceStub
from harmos._generated.proto.plugin.v1.stream_pb2 import SampleBatch, SamplesSelection, Window

# Local Imports
from harmos.plugin import Catalog, Context, Plugin, observer, publisher, window_observer
from tests.fixtures.conformance import Graph, Value, Values, plugin
from tests.support import calls

# Type Checking Imports

# === Public Exports === #

# === Constants === #

SECOND = Duration(seconds=1)
START = Timestamp(seconds=100)

# === Types === #

# === Classes === #


class WindowBatches(IsolatedAsyncioTestCase):
    def test_optional_values_keep_presence_through_split_windows(self) -> None:
        file = FileDescriptorProto(name="tests/optional.proto", package="tests.optional", syntax="proto3")
        row = file.message_type.add(name="Reading")
        row.oneof_decl.add(name="_value")
        row.field.add(
            name="value",
            number=1,
            label=FieldDescriptorProto.LABEL_OPTIONAL,
            type=FieldDescriptorProto.TYPE_DOUBLE,
            proto3_optional=True,
            oneof_index=0,
        )
        plain = file.message_type.add(name="PlainColumns")
        plain.field.add(name="value", number=1, label=FieldDescriptorProto.LABEL_REPEATED, type=FieldDescriptorProto.TYPE_DOUBLE)
        pool = DescriptorPool()
        pool.Add(file)
        reading = message_factory.GetMessageClass(pool.FindMessageTypeByName("tests.optional.Reading"))
        plain = message_factory.GetMessageClass(pool.FindMessageTypeByName("tests.optional.PlainColumns"))
        window_observer("example.optional", row=reading, columns=columns_type(reading, "example.optional"), interval=SECOND)
        self.assertRaises(ValueError, window_observer, "example.optional", row=reading, columns=plain, interval=SECOND)
        expected = [None, 0.0, 2.5, None, 0.0, -1.0]
        values: Columns = [[]]
        for value in expected:
            sample = reading(value=value) if value is not None else reading()
            append(values, sample)
        parts = batches(reading, values, len(expected), 58)
        self.assertGreater(len(parts), 1)
        actual = []
        for part in parts:
            columns = columns_type(reading, "example.optional").FromString(part.columns)
            actual.extend(extract(reading, columns, part.count)[0])
        self.assertEqual(actual, expected)
        self.assertEqual(values, [expected])

    async def test_one_sample_observer_answers_a_window_with_exact_columns(self) -> None:
        # Call the Observer Once per Sample, in Order, With Each Sample's Sequence.
        reading, inner, span = _reading()
        sequences: list[int] = []

        class Reader:
            def __init__(self) -> None:
                self.row = reading()

            @observer("example.reading", row=reading, interval=SECOND, window_setting="window")
            async def read(self, context: Context) -> Message:
                # One Reused Row Instance Still Yields Each Sample's Own Values.
                sequences.append(context.sequence)
                self.row.Clear()
                if context.sequence == 7:
                    self.row.MergeFrom(reading(gain=1.5, count=-1, label="a", inner=inner(x=1), span=span(seconds=2)))
                return self.row

        endpoint = await Plugin(id="example", version="1.0.0", register=lambda ctx: ctx.register(Catalog(Reader()))).start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.stop)
        channel = grpc.aio.insecure_channel(endpoint.address)
        self.addAsyncCleanup(channel.close)
        host = InstanceStub(channel)
        description = await host.Get(InstanceGetRequest(revision=1))
        self.assertEqual(description.declarations.streams[0].window_setting, "window")
        stream = host.Samples()
        self.addCleanup(stream.cancel)
        await stream.write(InstanceSamplesRequest(open=SamplesSelection(stream="example.reading")))
        await stream.write(InstanceSamplesRequest(window=Window(start=START, period=SECOND, count=2, sequence=7)))

        # Packed Numbers and One Entry per Sample, Defaults Included, Answer the Whole Window.
        columns = bytes.fromhex(
            "0a10" + "000000000000f83f" + "0000000000000000"  # gain: packed doubles
            "120b" + "ffffffffffffffffff01" + "00"  # count: packed int64
            "1a0161" + "1a00"  # label: one string per sample
            "22020801" + "2200"  # inner: one message per sample
            "2a020802" + "2a00"  # span: an imported message type
            "32020000"  # null: a packed imported enum
        )
        self.assertEqual((await stream.read()).batch, SampleBatch(count=2, columns=columns, last=True))
        self.assertEqual(sequences, [7, 8])

    async def test_window_observer_splits_a_million_samples_within_the_message_limit(self) -> None:
        # Grant the Conformance Window Observer One Million Samples.
        limit = MessageSettings().message_bytes
        endpoint = await plugin().start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.stop)
        channel = grpc.aio.insecure_channel(endpoint.address, options=[("grpc.max_receive_message_length", limit)])
        self.addAsyncCleanup(channel.close)
        host = InstanceStub(channel)
        self.assertEqual((await host.Get(InstanceGetRequest(revision=1))).message_bytes, limit)
        stream = host.Samples()
        self.addCleanup(stream.cancel)
        await stream.write(InstanceSamplesRequest(open=SamplesSelection(stream="conformance.window")))
        await stream.write(InstanceSamplesRequest(window=Window(start=START, period=SECOND, count=1_000_000, sequence=3)))

        # Contiguous Parts Each Fit the Advertised Limit and Only the Final One Answers.
        parts: list[InstanceSamplesResponse] = []
        while not parts or not parts[-1].batch.last:
            parts.append(await stream.read())
        self.assertGreater(len(parts), 1)
        values: list[bytes] = []
        for part in parts:
            self.assertLessEqual(part.ByteSize(), limit)
            self.assertEqual(part.batch.offset, len(values))
            self.assertGreaterEqual(part.batch.count, 1)
            column = Values.FromString(part.batch.columns).value
            self.assertEqual(len(column), part.batch.count)
            values.extend(column)
        self.assertEqual([part.batch.last for part in parts], [False] * (len(parts) - 1) + [True])
        self.assertEqual(values, [(3 + index).to_bytes(8, "little") for index in range(1_000_000)])

        # The Next Window Follows on the Same Call.
        await stream.write(InstanceSamplesRequest(window=Window(start=START, period=SECOND, count=2, sequence=1_000_003)))
        batch = (await stream.read()).batch
        self.assertEqual(
            batch,
            SampleBatch(
                count=2,
                columns=Values(value=[(1_000_003).to_bytes(8, "little"), (1_000_004).to_bytes(8, "little")]).SerializeToString(),
                last=True,
            ),
        )

    def test_splitting_keeps_every_part_within_a_small_limit(self) -> None:
        # Mixed Rows With Wide Varints, Strings and Messages Split at Sample Boundaries.
        reading, inner, _ = _reading()
        rows = [
            reading(gain=index / 3, count=-index if index % 2 else index * 1_000_003, label="x" * (index % 7), inner=inner(x=index))
            for index in range(50)
        ]
        columns: Columns = [[] for _ in reading.DESCRIPTOR.fields]
        for row in rows:
            append(columns, row)
        parts = batches(reading, columns, len(rows), 120)
        self.assertGreater(len(parts), 1)
        decoded = columns_type(reading, "example.reading")()
        offset = 0
        for part in parts:
            self.assertLessEqual(InstanceSamplesResponse(batch=part).ByteSize(), 120)
            self.assertEqual(part.offset, offset)
            self.assertGreaterEqual(part.count, 1)
            self.assertEqual(part.last, offset + part.count == len(rows))
            decoded.MergeFromString(part.columns)
            offset += part.count
        self.assertEqual(offset, len(rows))
        self.assertEqual(decoded.SerializeToString(), batches(reading, columns, len(rows), 2**31 - 1)[0].columns)

        # A Single Sample Beyond the Limit Fails the Window.
        wide: Columns = [[] for _ in reading.DESCRIPTOR.fields]
        append(wide, reading(label="x" * 200))
        self.assertRaises(ValueError, batches, reading, wide, 1, 120)

    async def test_windows_refuse_invalid_frames(self) -> None:
        # Serve the Conformance Fixture for Every Invalid Window.
        endpoint = await plugin().start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.stop)
        channel = grpc.aio.insecure_channel(endpoint.address)
        self.addAsyncCleanup(channel.close)
        host = InstanceStub(channel)

        # A Window Before Open or on a Device Stream Is Invalid.
        for name in ("", "conformance.device"):
            invalid = host.Samples()
            if name:
                await invalid.write(InstanceSamplesRequest(open=SamplesSelection(stream=name)))
                await invalid.read()
            await invalid.write(InstanceSamplesRequest(window=Window(start=START, period=SECOND, count=1, sequence=1)))
            await self._refused(invalid)

        # A Window Needs a Valid Start, a Positive Period and at Least One Sample.
        for window in (
            Window(period=SECOND, count=1, sequence=1),
            Window(start=Timestamp(seconds=0, nanos=-1), period=SECOND, count=1, sequence=1),
            Window(start=START, count=1, sequence=1),
            Window(start=START, period=Duration(), count=1, sequence=1),
            Window(start=START, period=Duration(seconds=-1), count=1, sequence=1),
            Window(start=START, period=SECOND, count=0, sequence=1),
        ):
            invalid = host.Samples()
            await invalid.write(InstanceSamplesRequest(open=SamplesSelection(stream="conformance.interval")))
            await invalid.write(InstanceSamplesRequest(window=window))
            await self._refused(invalid)

        # The Connection Stays Usable.
        call = await calls.invoke(host, "conformance.step.0", Value(value=b"after"))
        self.assertEqual(Value.FromString((await call.read()).response.payload).value, b"after")

    def test_declarations_refuse_rows_and_columns_without_one_value_per_field(self) -> None:
        # Interval Rows Hold One Plain Value per Field; Device Rows Are Unrestricted.
        for row in (Graph, struct_pb2.ListValue, Struct, struct_pb2.Value):
            with self.subTest(row=row.DESCRIPTOR.full_name):
                self.assertRaises(ValueError, observer, "example.reading", row=row, interval=SECOND)
        publisher("example.device", row=Graph)

        # Proto3 Optional Fields Use Synthetic Oneofs and Preserve Presence.
        observer("example.optional", row=Graph.Nested, interval=SECOND)

        # Window Columns Repeat Every Row Field With Its Number and Type.
        for columns in (Value, Graph, Struct):
            self.assertRaises(ValueError, window_observer, "example.window", row=Value, columns=columns, interval=SECOND)
        window_observer("example.window", row=Value, columns=Values, interval=SECOND)

    async def _refused(self, call: grpc.aio.StreamStreamCall) -> None:
        try:
            await call.read()
        except AioRpcError as error:
            self.assertEqual(error.code(), StatusCode.INVALID_ARGUMENT)
            return
        self.fail("Samples Accepted an Invalid Window")


# === Public Functions === #

# === Private Functions === #


def _reading() -> tuple[type[Message], type[Message], type[Message]]:
    """A Row With Double, Int64, String, Message and Imported Fields, Built in a Private Pool."""
    pool = DescriptorPool()
    for imported in (duration_pb2, struct_pb2):
        pool.AddSerializedFile(imported.DESCRIPTOR.serialized_pb)
    file = FileDescriptorProto(name="tests/window/reading.proto", package="example.window", syntax="proto3")
    file.dependency.extend([duration_pb2.DESCRIPTOR.name, struct_pb2.DESCRIPTOR.name])
    inner = file.message_type.add(name="Inner")
    inner.field.add(name="x", number=1, label=FieldDescriptorProto.LABEL_OPTIONAL, type=FieldDescriptorProto.TYPE_INT32)
    row = file.message_type.add(name="Reading")
    for name, number, kind in (
        ("gain", 1, FieldDescriptorProto.TYPE_DOUBLE),
        ("count", 2, FieldDescriptorProto.TYPE_INT64),
        ("label", 3, FieldDescriptorProto.TYPE_STRING),
    ):
        row.field.add(name=name, number=number, label=FieldDescriptorProto.LABEL_OPTIONAL, type=kind)
    row.field.add(
        name="inner",
        number=4,
        label=FieldDescriptorProto.LABEL_OPTIONAL,
        type=FieldDescriptorProto.TYPE_MESSAGE,
        type_name=".example.window.Inner",
    )
    row.field.add(
        name="span",
        number=5,
        label=FieldDescriptorProto.LABEL_OPTIONAL,
        type=FieldDescriptorProto.TYPE_MESSAGE,
        type_name=".google.protobuf.Duration",
    )
    row.field.add(
        name="null",
        number=6,
        label=FieldDescriptorProto.LABEL_OPTIONAL,
        type=FieldDescriptorProto.TYPE_ENUM,
        type_name=".google.protobuf.NullValue",
    )
    pool.Add(file)
    return (
        message_factory.GetMessageClass(pool.FindMessageTypeByName("example.window.Reading")),
        message_factory.GetMessageClass(pool.FindMessageTypeByName("example.window.Inner")),
        message_factory.GetMessageClass(pool.FindMessageTypeByName("google.protobuf.Duration")),
    )


# === Entry Point === #
