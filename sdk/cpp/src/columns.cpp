/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports
#include <algorithm>
#include <stdexcept>

// Third Party Imports
#include <google/protobuf/io/coded_stream.h>
#include <google/protobuf/wire_format_lite.h>

// Local Imports
#include "columns.hpp"
#include "harmos/harmos.hpp"

using google::protobuf::FieldDescriptor;
using google::protobuf::Message;
using google::protobuf::internal::WireFormatLite;
using google::protobuf::io::CodedOutputStream;
using std::invalid_argument;
using std::size_t;
using std::string;
using std::to_string;
using std::uint64_t;
using std::uint8_t;

/* === Macros === */

// The Windows SDK Renames GetMessage; Reflection Needs Its Own Name.
#ifdef GetMessage
#undef GetMessage
#endif

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

namespace {
void append_varint(string &output, uint64_t value) {
  uint8_t scratch[10];
  const auto *end = CodedOutputStream::WriteVarint64ToArray(value, scratch);
  output.append(reinterpret_cast<const char *>(scratch), static_cast<size_t>(end - scratch));
}

string type_name(const FieldDescriptor *field) {
  if (field->message_type())
    return string(field->message_type()->full_name());
  if (field->enum_type())
    return string(field->enum_type()->full_name());
  return string(field->type_name());
}
} // namespace

namespace harmos::plugin::detail {
void validate_row(const google::protobuf::Descriptor *row) {
  for (int index = 0; index < row->field_count(); ++index) {
    const auto *field = row->field(index);
    const auto declare = "Declare Observer " + string(row->full_name()) + ": Make Field " + string(field->name()) + " ";
    if (field->is_repeated())
      throw invalid_argument(declare + "Singular; an Interval Row Holds One Value per Field, Not Repeated or Map Fields");
    if (field->containing_oneof())
      throw invalid_argument(declare + "a Plain Field; an Interval Row Refuses Oneof and Optional Fields");
    if (field->type() == FieldDescriptor::TYPE_GROUP)
      throw invalid_argument(declare + "a Message Field; an Interval Row Refuses Groups");
  }
}

void validate_columns(const google::protobuf::Descriptor *row, const google::protobuf::Descriptor *columns) {
  validate_row(row);
  const auto declare = "Declare Window Observer " + string(columns->full_name()) + ": ";
  if (columns->field_count() != row->field_count())
    throw invalid_argument(declare + "Declare Exactly the Fields of " + string(row->full_name()) + ", Each Repeated");
  for (int index = 0; index < row->field_count(); ++index) {
    const auto *field = row->field(index);
    const auto *column = columns->FindFieldByNumber(field->number());
    if (!column || !column->is_repeated() || column->is_map() || column->type() != field->type() || type_name(column) != type_name(field))
      throw invalid_argument(declare + "Declare Field " + to_string(field->number()) + " as repeated " + type_name(field) + " to Mirror " +
                             string(row->full_name()) + "." + string(field->name()));
  }
}

Columns::Columns(const google::protobuf::Descriptor *row) : row_(row) {
  validate_row(row);
  for (int index = 0; index < row->field_count(); ++index) {
    const auto *field = row->field(index);
    Column column;
    column.field = field;
    column.packed = field->cpp_type() != FieldDescriptor::CPPTYPE_STRING && field->cpp_type() != FieldDescriptor::CPPTYPE_MESSAGE;
    append_varint(column.tag, WireFormatLite::MakeTag(field->number(), WireFormatLite::WIRETYPE_LENGTH_DELIMITED));
    column.ends.push_back(0);
    columns_.push_back(std::move(column));
  }
  // Serialized Messages Order Fields by Number.
  std::sort(columns_.begin(), columns_.end(), [](const Column &left, const Column &right) {
    return left.field->number() < right.field->number();
  });
}

void Columns::write(Column &column, const Message &message, const FieldDescriptor *field, int index) {
  const auto *reflection = message.GetReflection();
  const bool repeated = index >= 0;
  uint8_t scratch[10];
  uint8_t *end = scratch;
  switch (field->type()) {
  case FieldDescriptor::TYPE_DOUBLE:
    end = WireFormatLite::WriteDoubleNoTagToArray(
        repeated ? reflection->GetRepeatedDouble(message, field, index) : reflection->GetDouble(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_FLOAT:
    end = WireFormatLite::WriteFloatNoTagToArray(
        repeated ? reflection->GetRepeatedFloat(message, field, index) : reflection->GetFloat(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_INT64:
    end = WireFormatLite::WriteInt64NoTagToArray(
        repeated ? reflection->GetRepeatedInt64(message, field, index) : reflection->GetInt64(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_SINT64:
    end = WireFormatLite::WriteSInt64NoTagToArray(
        repeated ? reflection->GetRepeatedInt64(message, field, index) : reflection->GetInt64(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_SFIXED64:
    end = WireFormatLite::WriteSFixed64NoTagToArray(
        repeated ? reflection->GetRepeatedInt64(message, field, index) : reflection->GetInt64(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_UINT64:
    end = WireFormatLite::WriteUInt64NoTagToArray(
        repeated ? reflection->GetRepeatedUInt64(message, field, index) : reflection->GetUInt64(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_FIXED64:
    end = WireFormatLite::WriteFixed64NoTagToArray(
        repeated ? reflection->GetRepeatedUInt64(message, field, index) : reflection->GetUInt64(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_INT32:
    end = WireFormatLite::WriteInt32NoTagToArray(
        repeated ? reflection->GetRepeatedInt32(message, field, index) : reflection->GetInt32(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_SINT32:
    end = WireFormatLite::WriteSInt32NoTagToArray(
        repeated ? reflection->GetRepeatedInt32(message, field, index) : reflection->GetInt32(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_SFIXED32:
    end = WireFormatLite::WriteSFixed32NoTagToArray(
        repeated ? reflection->GetRepeatedInt32(message, field, index) : reflection->GetInt32(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_UINT32:
    end = WireFormatLite::WriteUInt32NoTagToArray(
        repeated ? reflection->GetRepeatedUInt32(message, field, index) : reflection->GetUInt32(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_FIXED32:
    end = WireFormatLite::WriteFixed32NoTagToArray(
        repeated ? reflection->GetRepeatedUInt32(message, field, index) : reflection->GetUInt32(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_BOOL:
    end = WireFormatLite::WriteBoolNoTagToArray(
        repeated ? reflection->GetRepeatedBool(message, field, index) : reflection->GetBool(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_ENUM:
    end = WireFormatLite::WriteEnumNoTagToArray(
        repeated ? reflection->GetRepeatedEnumValue(message, field, index) : reflection->GetEnumValue(message, field), scratch);
    break;
  case FieldDescriptor::TYPE_STRING:
  case FieldDescriptor::TYPE_BYTES: {
    string buffer;
    const auto &value = repeated ? reflection->GetRepeatedStringReference(message, field, index, &buffer)
                                 : reflection->GetStringReference(message, field, &buffer);
    append_varint(column.values, value.size());
    column.values += value;
    column.ends.push_back(column.values.size());
    return;
  }
  case FieldDescriptor::TYPE_MESSAGE: {
    const auto value =
        detail::encode(repeated ? reflection->GetRepeatedMessage(message, field, index) : reflection->GetMessage(message, field));
    append_varint(column.values, value.size());
    column.values += value;
    column.ends.push_back(column.values.size());
    return;
  }
  case FieldDescriptor::TYPE_GROUP:
    throw invalid_argument("Encode Columns: Declare Field " + string(field->name()) + " as a Message Instead of a Group");
  }
  column.values.append(reinterpret_cast<const char *>(scratch), static_cast<size_t>(end - scratch));
  column.ends.push_back(column.values.size());
}

void Columns::append(const Message &row) {
  if (row.GetDescriptor() != row_)
    throw invalid_argument("Answer Sample: Return a " + string(row_->full_name()) + " Row");
  for (auto &column : columns_)
    write(column, row, column.field, -1);
  ++count_;
}

void Columns::extend(const Message &columns, uint64_t count) {
  const auto *descriptor = columns.GetDescriptor();
  const auto *reflection = columns.GetReflection();
  for (auto &column : columns_) {
    const auto *field = descriptor->FindFieldByNumber(column.field->number());
    const auto size = static_cast<uint64_t>(reflection->FieldSize(columns, field));
    if (size != count)
      throw invalid_argument("Answer Window: Return " + to_string(count) + " Values in Column " + string(field->name()) + " of " +
                             string(descriptor->full_name()) + ", One per Sample; Got " + to_string(size));
  }
  for (auto &column : columns_) {
    const auto *field = descriptor->FindFieldByNumber(column.field->number());
    for (uint64_t index = 0; index < count; ++index)
      write(column, columns, field, static_cast<int>(index));
  }
  count_ += count;
}

size_t Columns::size(uint64_t begin, uint64_t end) const {
  size_t total = 0;
  for (const auto &column : columns_) {
    const auto bytes = column.ends[end] - column.ends[begin];
    if (column.packed)
      total += column.tag.size() + CodedOutputStream::VarintSize64(bytes) + bytes;
    else
      total += static_cast<size_t>(end - begin) * column.tag.size() + bytes;
  }
  return total;
}

string Columns::encode(uint64_t begin, uint64_t end) const {
  string output;
  output.reserve(size(begin, end));
  for (const auto &column : columns_) {
    if (column.packed) {
      output += column.tag;
      append_varint(output, column.ends[end] - column.ends[begin]);
      output.append(column.values, column.ends[begin], column.ends[end] - column.ends[begin]);
      continue;
    }
    for (auto index = begin; index < end; ++index) {
      output += column.tag;
      output.append(column.values, column.ends[index], column.ends[index + 1] - column.ends[index]);
    }
  }
  return output;
}

std::vector<std::pair<uint64_t, uint64_t>> Columns::parts(size_t limit) const {
  std::vector<std::pair<uint64_t, uint64_t>> result;
  uint64_t begin = 0;
  while (begin < count_) {
    auto end = begin;
    while (end < count_ && framed_size(begin, end + 1 - begin, size(begin, end + 1), end + 1 == count_) <= limit)
      ++end;
    if (end == begin) {
      const auto needed = framed_size(begin, 1, size(begin, begin + 1), begin + 1 == count_);
      throw TransportError(8, "Answer Window: Shrink " + string(row_->full_name()) + " Rows to Fit the " + to_string(limit) +
                                  "-Byte Message Limit; Sample " + to_string(begin) + " Needs " + to_string(needed) + " Bytes");
    }
    result.emplace_back(begin, end);
    begin = end;
  }
  return result;
}

size_t framed_size(uint64_t offset, uint64_t count, size_t columns, bool last) {
  size_t batch = 0;
  if (offset)
    batch += 1 + CodedOutputStream::VarintSize64(offset);
  if (count)
    batch += 1 + CodedOutputStream::VarintSize64(count);
  if (columns)
    batch += 1 + CodedOutputStream::VarintSize64(columns) + columns;
  if (last)
    batch += 2;
  return 1 + CodedOutputStream::VarintSize64(batch) + batch;
}
} // namespace harmos::plugin::detail

/* === Private Functions === */

/* === Entry Point === */
