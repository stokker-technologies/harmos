/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#pragma once

/* === Imports === */

// Standard Library Imports
#include <cstddef>
#include <cstdint>
#include <string>
#include <utility>
#include <vector>

// Third Party Imports
#include <google/protobuf/descriptor.h>
#include <google/protobuf/message.h>

// Local Imports

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

namespace harmos::plugin::detail {
// One Window's Samples as Row Columns: Each Row Field Keeps Its Encoded Values and Per-Sample Ends.
class Columns {
public:
  explicit Columns(const google::protobuf::Descriptor *row);
  // Appends One Sample from a Row of the Stream's Type.
  void append(const google::protobuf::Message &row);
  // Appends Every Sample of a Columns Message That Repeats Each Row Field `count` Times.
  void extend(const google::protobuf::Message &columns, std::uint64_t count);
  std::uint64_t count() const {
    return count_;
  }
  // Encoded Size of the Columns Message Holding Samples [begin, end).
  std::size_t size(std::uint64_t begin, std::uint64_t end) const;
  // The Columns Message Holding Samples [begin, end): Numeric Columns Packed, Others One Entry per Sample.
  std::string encode(std::uint64_t begin, std::uint64_t end) const;
  // Splits All Samples into [begin, end) Parts Whose Framed InstanceSamplesResponse Fits `limit` Bytes.
  std::vector<std::pair<std::uint64_t, std::uint64_t>> parts(std::size_t limit) const;

private:
  struct Column {
    const google::protobuf::FieldDescriptor *field = nullptr;
    std::string tag;
    bool packed = false;
    std::string values;
    std::vector<std::size_t> ends;
  };
  const google::protobuf::Descriptor *row_;
  std::vector<Column> columns_;
  std::uint64_t count_ = 0;

  static void write(Column &column, const google::protobuf::Message &message, const google::protobuf::FieldDescriptor *field, int index);
};

// Encoded Size of an InstanceSamplesResponse Carrying One SampleBatch Part.
std::size_t framed_size(std::uint64_t offset, std::uint64_t count, std::size_t columns, bool last);
} // namespace harmos::plugin::detail

/* === Templates === */

/* === Public Declarations === */
