// SPDX-License-Identifier: Apache-2.0

using System.Collections.Generic;
using System.Collections;
using System.Linq;
using System;
using Google.Protobuf.Reflection;
using Google.Protobuf.WellKnownTypes;
using Google.Protobuf;
using Grpc.Core;
using Harmos.Internal.Plugin;
using Status = Grpc.Core.Status;

namespace Harmos.Plugins;

/// <summary>One host-granted interval window: Count samples, the first at Start, each one Period later.</summary>
/// <param name="Sequence">One-based sequence of the window's first sample within the Samples call.</param>
public sealed record Window(Timestamp Start, Duration Period, ulong Count, ulong Sequence)
{
  internal Timestamp SampleTime(ulong index)
  {
    var nanos = (Int128)Start.Seconds * 1_000_000_000 + Start.Nanos + ((Int128)Period.Seconds * 1_000_000_000 + Period.Nanos) * index;
    var seconds = nanos / 1_000_000_000;
    var remainder = nanos % 1_000_000_000;
    if (remainder < 0)
    {
      seconds -= 1;
      remainder += 1_000_000_000;
    }
    return new Timestamp { Seconds = (long)seconds, Nanos = (int)remainder };
  }
}

/// <summary>The values of one answered window, one list per row field, encoded and split into batch parts at sample boundaries.</summary>
internal sealed class Columns
{
  private readonly string row;
  private readonly FieldDescriptor[] fields;
  private readonly object?[][] values;
  private readonly int count;

  private Columns(string row, FieldDescriptor[] fields, object?[][] values, int count)
  {
    this.row = row;
    this.fields = fields;
    this.values = values;
    this.count = count;
  }

  /// <summary>Refuse rows whose fields cannot hold exactly one value per sample.</summary>
  internal static FieldDescriptor[] RowFields(MessageDescriptor row, string stream)
  {
    var fields = row.Fields.InFieldNumberOrder().ToArray();
    foreach (var field in fields)
      if (field.IsMap || field.IsRepeated || field.ContainingOneof is not null || field.FieldType == FieldType.Group)
        throw new ArgumentException($"Declare {stream}: Row {row.FullName} Field {field.Name} Must Hold One Value per Sample; " +
            "Remove Repeated, Map, Oneof and Optional Fields from Interval Rows");
    return fields;
  }

  /// <summary>Refuse columns that do not repeat every row field with the same number and type.</summary>
  internal static FieldDescriptor[] Mirror(MessageDescriptor row, MessageDescriptor columns, string stream)
  {
    var fields = RowFields(row, stream);
    foreach (var column in columns.Fields.InFieldNumberOrder())
    {
      var field = row.FindFieldByNumber(column.FieldNumber);
      if (field is null || column.IsMap || !column.IsRepeated || column.FieldType != field.FieldType || TypeName(column) != TypeName(field))
        throw new ArgumentException($"Declare {stream}: Columns {columns.FullName} Field {column.Name} Must Repeat " +
            $"Row {row.FullName} Field {column.FieldNumber} with the Same Type");
    }
    foreach (var field in fields)
      if (columns.FindFieldByNumber(field.FieldNumber) is null)
        throw new ArgumentException($"Declare {stream}: Columns {columns.FullName} Must Repeat Row {row.FullName} " +
            $"Field {field.Name} as Field {field.FieldNumber}");
    return fields;
  }

  /// <summary>Samples one window can hold in memory.</summary>
  internal static int Capacity(Window window, string stream)
  {
    if (window.Count > (ulong)Array.MaxLength)
      throw new RpcException(new Status(StatusCode.ResourceExhausted,
          $"Observe {stream}: A Window of {window.Count} Samples Exceeds {Array.MaxLength}; Shorten the Stream's Window Setting"));
    return (int)window.Count;
  }

  internal static Columns FromRows(MessageDescriptor row, FieldDescriptor[] fields, IMessage[] rows)
  {
    var values = new object?[fields.Length][];
    for (var column = 0; column < fields.Length; column++)
    {
      var field = fields[column];
      values[column] = new object?[rows.Length];
      for (var sample = 0; sample < rows.Length; sample++)
        values[column][sample] = Normalize(field, field.Accessor.GetValue(rows[sample]));
    }
    return new Columns(row.FullName, fields, values, rows.Length);
  }

  internal static Columns FromColumns(MessageDescriptor row, FieldDescriptor[] fields, IMessage columns, Window window, string stream)
  {
    var count = Capacity(window, stream);
    var values = new object?[fields.Length][];
    for (var column = 0; column < fields.Length; column++)
    {
      var field = fields[column];
      var list = (IList)columns.Descriptor.FindFieldByNumber(field.FieldNumber).Accessor.GetValue(columns);
      if (list.Count != count)
        throw Failed($"Observe {stream}: Columns Field {field.Name} Holds {list.Count} Values for {count} Samples; Return One Value per Sample");
      values[column] = new object?[count];
      for (var sample = 0; sample < count; sample++)
        values[column][sample] = Normalize(field, list[sample]);
    }
    return new Columns(row.FullName, fields, values, count);
  }

  /// <summary>A plugin-side window failure; the message names the author's action.</summary>
  internal static RpcException Failed(string message) => new(new Status(StatusCode.Internal, message));

  /// <summary>Split the window into parts whose framed samples response fits the message limit.</summary>
  internal IEnumerable<SampleBatch> Parts(int limit, string stream)
  {
    var payload = new long[fields.Length];
    var sizes = new long[fields.Length];
    var start = 0;
    for (var sample = 0; sample < count; sample++)
    {
      for (var column = 0; column < fields.Length; column++)
      {
        sizes[column] = EntrySize(column, sample);
        payload[column] += sizes[column];
      }
      if (Framed(payload, start, sample + 1 - start, sample + 1 == count) <= limit)
        continue;
      if (sample == start)
        throw TooLarge(stream, Framed(payload, start, 1, sample + 1 == count), limit);
      yield return Part(payload, sizes, start, sample - start, false);
      start = sample;
      Array.Copy(sizes, payload, sizes.Length);
      if (Framed(payload, start, 1, sample + 1 == count) > limit)
        throw TooLarge(stream, Framed(payload, start, 1, sample + 1 == count), limit);
    }
    yield return Part(payload, sizes, start, count - start, true);
  }

  private RpcException TooLarge(string stream, long needed, int limit) => new(new Status(StatusCode.ResourceExhausted,
      $"Answer {stream}: One {row} Sample Needs {needed} Bytes but the Plugin Message Limit Is {limit}; " +
      "Raise the Plugin's message_bytes or Shrink the Row"));

  private SampleBatch Part(long[] payload, long[] sizes, int start, int samples, bool last)
  {
    // The Caller's Running Sums May Include the Next Part's First Sample.
    var lengths = (long[])payload.Clone();
    if (!last)
      for (var column = 0; column < fields.Length; column++)
        lengths[column] -= sizes[column];
    var bytes = new byte[ColumnBytes(lengths)];
    var output = new CodedOutputStream(bytes);
    for (var column = 0; column < fields.Length; column++)
    {
      var field = fields[column];
      if (Packed(field))
      {
        output.WriteTag(field.FieldNumber, WireFormat.WireType.LengthDelimited);
        output.WriteLength(checked((int)lengths[column]));
        for (var sample = start; sample < start + samples; sample++)
          Write(output, field, values[column][sample]);
        continue;
      }
      for (var sample = start; sample < start + samples; sample++)
      {
        output.WriteTag(field.FieldNumber, WireFormat.WireType.LengthDelimited);
        Write(output, field, values[column][sample]);
      }
    }
    output.Flush();
    output.CheckNoSpaceLeft();
    return new SampleBatch { Offset = (ulong)start, Count = (ulong)samples, Columns = UnsafeByteOperations.UnsafeWrap(bytes), Last = last };
  }

  private long Framed(long[] payload, int start, int samples, bool last)
  {
    var columns = ColumnBytes(payload);
    var batch = (start == 0 ? 0 : 1 + VarintSize((ulong)start)) + 1 + VarintSize((ulong)samples) +
        (columns == 0 ? 0 : 1 + VarintSize((ulong)columns) + columns) + (last ? 2 : 0);
    return 1 + VarintSize((ulong)batch) + batch;
  }

  private long ColumnBytes(long[] payload)
  {
    long total = 0;
    for (var column = 0; column < fields.Length; column++)
      total += Packed(fields[column])
          ? CodedOutputStream.ComputeTagSize(fields[column].FieldNumber) + VarintSize((ulong)payload[column]) + payload[column]
          : payload[column];
    return total;
  }

  /// <summary>A packed value's size, or a length-delimited entry's size including its tag.</summary>
  private long EntrySize(int column, int sample)
  {
    var field = fields[column];
    var value = values[column][sample];
    return field.FieldType switch
    {
      FieldType.Double or FieldType.Fixed64 or FieldType.SFixed64 => 8,
      FieldType.Float or FieldType.Fixed32 or FieldType.SFixed32 => 4,
      FieldType.Bool => 1,
      FieldType.Int64 => CodedOutputStream.ComputeInt64Size((long)value!),
      FieldType.UInt64 => CodedOutputStream.ComputeUInt64Size((ulong)value!),
      FieldType.Int32 => CodedOutputStream.ComputeInt32Size((int)value!),
      FieldType.UInt32 => CodedOutputStream.ComputeUInt32Size((uint)value!),
      FieldType.SInt32 => CodedOutputStream.ComputeSInt32Size((int)value!),
      FieldType.SInt64 => CodedOutputStream.ComputeSInt64Size((long)value!),
      FieldType.Enum => CodedOutputStream.ComputeEnumSize(Convert.ToInt32(value)),
      FieldType.String => CodedOutputStream.ComputeTagSize(field.FieldNumber) + CodedOutputStream.ComputeStringSize((string)value!),
      FieldType.Bytes => CodedOutputStream.ComputeTagSize(field.FieldNumber) + CodedOutputStream.ComputeBytesSize((ByteString)value!),
      _ => CodedOutputStream.ComputeTagSize(field.FieldNumber) +
          (value is IMessage message ? CodedOutputStream.ComputeMessageSize(message) : CodedOutputStream.ComputeLengthSize(0))
    };
  }

  private static void Write(CodedOutputStream output, FieldDescriptor field, object? value)
  {
    switch (field.FieldType)
    {
      case FieldType.Double:
        output.WriteDouble((double)value!);
        break;
      case FieldType.Float:
        output.WriteFloat((float)value!);
        break;
      case FieldType.Int64:
        output.WriteInt64((long)value!);
        break;
      case FieldType.UInt64:
        output.WriteUInt64((ulong)value!);
        break;
      case FieldType.Int32:
        output.WriteInt32((int)value!);
        break;
      case FieldType.Fixed64:
        output.WriteFixed64((ulong)value!);
        break;
      case FieldType.Fixed32:
        output.WriteFixed32((uint)value!);
        break;
      case FieldType.Bool:
        output.WriteBool((bool)value!);
        break;
      case FieldType.UInt32:
        output.WriteUInt32((uint)value!);
        break;
      case FieldType.SFixed32:
        output.WriteSFixed32((int)value!);
        break;
      case FieldType.SFixed64:
        output.WriteSFixed64((long)value!);
        break;
      case FieldType.SInt32:
        output.WriteSInt32((int)value!);
        break;
      case FieldType.SInt64:
        output.WriteSInt64((long)value!);
        break;
      case FieldType.Enum:
        output.WriteEnum(Convert.ToInt32(value));
        break;
      case FieldType.String:
        output.WriteString((string)value!);
        break;
      case FieldType.Bytes:
        output.WriteBytes((ByteString)value!);
        break;
      default:
        if (value is IMessage message)
          output.WriteMessage(message);
        else
          output.WriteLength(0);
        break;
    }
  }

  /// <summary>Wrap unwrapped well-known wrapper values so every message column holds a message or null.</summary>
  private static object? Normalize(FieldDescriptor field, object? value)
  {
    if (field.FieldType != FieldType.Message || value is null or IMessage)
      return value;
    var wrapper = field.MessageType.Parser.ParseFrom(ByteString.Empty);
    field.MessageType.FindFieldByNumber(1).Accessor.SetValue(wrapper, value);
    return wrapper;
  }

  private static bool Packed(FieldDescriptor field) => field.FieldType is not (FieldType.String or FieldType.Bytes or FieldType.Message);

  private static string TypeName(FieldDescriptor field) => field.FieldType switch
  {
    FieldType.Message => field.MessageType.FullName,
    FieldType.Enum => field.EnumType.FullName,
    _ => ""
  };

  private static int VarintSize(ulong value) => CodedOutputStream.ComputeUInt64Size(value);
}
