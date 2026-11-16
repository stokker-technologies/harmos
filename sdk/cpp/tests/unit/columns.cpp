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
#include <memory>

// Third Party Imports
#include <catch2/catch_test_macros.hpp>
#include <google/protobuf/descriptor.h>
#include <google/protobuf/descriptor.pb.h>
#include <google/protobuf/duration.pb.h>
#include <google/protobuf/dynamic_message.h>

// Local Imports
#include "columns.hpp"
#include "generated/types.hpp"
#include "harmos/harmos.hpp"

namespace authoring = harmos::plugin;
using google::protobuf::DescriptorPool;
using google::protobuf::DescriptorProto;
using google::protobuf::Duration;
using google::protobuf::DynamicMessageFactory;
using google::protobuf::FieldDescriptorProto;
using google::protobuf::FileDescriptor;
using google::protobuf::FileDescriptorProto;
using google::protobuf::Message;
using std::invalid_argument;
using std::string;
using std::unique_ptr;
using std::vector;

/* === Macros === */

// The Windows SDK Renames GetMessage; Reflection Needs Its Own Name.
#ifdef GetMessage
#undef GetMessage
#endif

/* === Constants === */

/* === Types === */

namespace {
using Value = harmos::conformance::Value;

/* === Classes === */

// A Runtime-Declared Row of Double, Int64, String and Message Fields with Its Columns.
class Schema {
public:
  DescriptorPool pool{DescriptorPool::generated_pool()};
  DynamicMessageFactory factory{&pool};
  const FileDescriptor *file = nullptr;

  Schema() {
    factory.SetDelegateToGeneratedFactory(true);
    FileDescriptorProto proto;
    proto.set_name("example/columns.proto");
    proto.set_package("example");
    proto.set_syntax("proto3");
    proto.add_dependency(string(Duration::descriptor()->file()->name()));
    for (const auto *name : {"Row", "Columns", "Mismatched", "Extra"}) {
      auto *message = proto.add_message_type();
      message->set_name(name);
      const bool repeated = string(name) != "Row";
      // Declaration Order Differs from Number Order; Columns Follow Numbers.
      field(message, "label", 3, FieldDescriptorProto::TYPE_STRING, repeated);
      field(message, "reading", 1, FieldDescriptorProto::TYPE_DOUBLE, repeated);
      field(message, "count", 2, string(name) == "Mismatched" ? FieldDescriptorProto::TYPE_INT32 : FieldDescriptorProto::TYPE_INT64,
            repeated);
      field(message, "span", 4, FieldDescriptorProto::TYPE_MESSAGE, repeated, ".google.protobuf.Duration");
      if (string(name) == "Extra")
        field(message, "extra", 5, FieldDescriptorProto::TYPE_BOOL, true);
    }
    file = pool.BuildFile(proto);
    if (!file)
      throw std::runtime_error("Declare Columns Schema: Build Failed");
  }
  unique_ptr<Message> make(const char *name) {
    return unique_ptr<Message>(factory.GetPrototype(file->FindMessageTypeByName(name))->New());
  }
  unique_ptr<Message> row(double reading, std::int64_t count, const string &label, const Duration *span) {
    auto result = make("Row");
    const auto *descriptor = result->GetDescriptor();
    const auto *reflection = result->GetReflection();
    reflection->SetDouble(result.get(), descriptor->FindFieldByName("reading"), reading);
    reflection->SetInt64(result.get(), descriptor->FindFieldByName("count"), count);
    reflection->SetString(result.get(), descriptor->FindFieldByName("label"), label);
    if (span)
      reflection->MutableMessage(result.get(), descriptor->FindFieldByName("span"), &factory)->CopyFrom(*span);
    return result;
  }
  // Protobuf's Own Serialization of the Columns Message for the Given Rows.
  unique_ptr<Message> columns(const vector<unique_ptr<Message>> &rows, std::size_t begin, std::size_t end) {
    auto result = make("Columns");
    const auto *descriptor = result->GetDescriptor();
    const auto *reflection = result->GetReflection();
    for (auto index = begin; index < end; ++index) {
      const auto &row = *rows[index];
      const auto *source = row.GetDescriptor();
      const auto *values = row.GetReflection();
      reflection->AddDouble(result.get(), descriptor->FindFieldByName("reading"),
                            values->GetDouble(row, source->FindFieldByName("reading")));
      reflection->AddInt64(result.get(), descriptor->FindFieldByName("count"), values->GetInt64(row, source->FindFieldByName("count")));
      reflection->AddString(result.get(), descriptor->FindFieldByName("label"), values->GetString(row, source->FindFieldByName("label")));
      reflection->AddMessage(result.get(), descriptor->FindFieldByName("span"), &factory)
          ->CopyFrom(values->GetMessage(row, source->FindFieldByName("span"), &factory));
    }
    return result;
  }

private:
  static void field(DescriptorProto *message, const char *name, int number, FieldDescriptorProto::Type type, bool repeated,
                    const char *type_name = nullptr) {
    auto *field = message->add_field();
    field->set_name(name);
    field->set_number(number);
    field->set_type(type);
    field->set_label(repeated ? FieldDescriptorProto::LABEL_REPEATED : FieldDescriptorProto::LABEL_OPTIONAL);
    if (type_name)
      field->set_type_name(type_name);
  }
};

/* === Templates === */

/* === Public Functions === */

/* === Private Functions === */

/* === Entry Point === */

} // namespace

TEST_CASE("Columns Encode Exactly as the Row's Repeated Columns Message") {
  Schema schema;
  Duration span;
  span.set_seconds(2);
  span.set_nanos(5);
  vector<unique_ptr<Message>> rows;
  rows.push_back(schema.row(1.5, -3, "a", &span));
  rows.push_back(schema.row(0, 0, "", nullptr));
  rows.push_back(schema.row(-0.25, std::int64_t{1} << 40, "xyz", &span));
  const auto *descriptor = schema.file->FindMessageTypeByName("Row");

  SECTION("Rows Transpose into Packed Numerics and One Entry per Sample, Defaults Included") {
    authoring::detail::Columns columns(descriptor);
    for (const auto &row : rows)
      columns.append(*row);
    REQUIRE(columns.count() == 3);
    REQUIRE(columns.encode(0, 3) == authoring::detail::encode(*schema.columns(rows, 0, 3)));
    REQUIRE(columns.encode(1, 3) == authoring::detail::encode(*schema.columns(rows, 1, 3)));
    REQUIRE(columns.encode(1, 2) == authoring::detail::encode(*schema.columns(rows, 1, 2)));
    REQUIRE(columns.size(0, 3) == columns.encode(0, 3).size());
    REQUIRE(columns.size(2, 3) == columns.encode(2, 3).size());
  }
  SECTION("A Columns Message Encodes the Same Bytes") {
    const auto expected = schema.columns(rows, 0, 3);
    authoring::detail::Columns columns(descriptor);
    columns.extend(*expected, 3);
    REQUIRE(columns.encode(0, 3) == authoring::detail::encode(*expected));
    authoring::detail::Columns short_columns(descriptor);
    REQUIRE_THROWS_AS(short_columns.extend(*expected, 4), invalid_argument);
  }
  SECTION("Bytes Rows Keep Empty Values") {
    authoring::detail::Columns columns(Value::descriptor());
    Value first;
    first.set_value("a");
    columns.append(first);
    columns.append(Value{});
    REQUIRE(columns.encode(0, 2) == string("\x0a\x01"
                                           "a"
                                           "\x0a\x00",
                                           5));
  }
}

TEST_CASE("Window Columns Must Repeat Every Row Field with Its Number and Type") {
  Schema schema;
  const auto *row = schema.file->FindMessageTypeByName("Row");
  REQUIRE_NOTHROW(authoring::detail::validate_columns(row, schema.file->FindMessageTypeByName("Columns")));
  REQUIRE_THROWS_AS(authoring::detail::validate_columns(row, row), invalid_argument);
  REQUIRE_THROWS_AS(authoring::detail::validate_columns(row, schema.file->FindMessageTypeByName("Mismatched")), invalid_argument);
  REQUIRE_THROWS_AS(authoring::detail::validate_columns(row, schema.file->FindMessageTypeByName("Extra")), invalid_argument);
  REQUIRE_THROWS_AS(authoring::detail::validate_row(schema.file->FindMessageTypeByName("Columns")), invalid_argument);
}

TEST_CASE("Parts Fit the Message Limit at Sample Boundaries") {
  authoring::detail::Columns columns(Value::descriptor());
  for (int index = 0; index < 40; ++index) {
    Value row;
    row.set_value(string(static_cast<std::size_t>(index % 7), 'v'));
    columns.append(row);
  }
  const std::size_t limit = 48;
  const auto parts = columns.parts(limit);
  REQUIRE(parts.size() > 1);
  std::uint64_t next = 0;
  for (const auto &[begin, end] : parts) {
    REQUIRE(begin == next);
    REQUIRE(end > begin);
    REQUIRE(authoring::detail::framed_size(begin, end - begin, columns.size(begin, end), end == columns.count()) <= limit);
    next = end;
  }
  REQUIRE(next == columns.count());
  REQUIRE_THROWS_AS(columns.parts(8), harmos::TransportError);
}
