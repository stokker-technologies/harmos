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
#include <map>
#include <set>

// Third Party Imports
#include <google/protobuf/io/coded_stream.h>
#include <google/protobuf/io/zero_copy_stream_impl_lite.h>
#include <openssl/sha.h>

// Local Imports
#include "harmos/harmos.hpp"

using google::protobuf::Descriptor;
using google::protobuf::DescriptorProto;
using google::protobuf::FileDescriptorProto;
using google::protobuf::FileDescriptorSet;
using google::protobuf::Message;
using std::invalid_argument;
using std::map;
using std::set;
using std::string;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

namespace harmos::plugin::detail {
string encode(const Message &message) {
  string bytes;
  google::protobuf::io::StringOutputStream stream(&bytes);
  {
    google::protobuf::io::CodedOutputStream output(&stream);
    output.SetSerializationDeterministic(true);
    if (!message.SerializeToCodedStream(&output))
      throw invalid_argument("Protobuf Encoding Failed");
  }
  return bytes;
}

string sha256(const string &bytes) {
  unsigned char digest[SHA256_DIGEST_LENGTH];
  SHA256(reinterpret_cast<const unsigned char *>(bytes.data()), bytes.size(), digest);
  return {reinterpret_cast<const char *>(digest), sizeof(digest)};
}

void merge(FileDescriptorSet &target, const FileDescriptorSet &source) {
  for (const auto &file : source.file()) {
    FileDescriptorProto *found = nullptr;
    for (auto &existing : *target.mutable_file()) {
      if (existing.name() != file.name())
        continue;
      found = &existing;
    }
    if (!found) {
      *target.add_file() = file;
      continue;
    }
    if (found->package() != file.package() || found->syntax() != file.syntax()) {
      throw invalid_argument("Conflicting Protobuf File: " + file.name());
    }
    for (const auto &dependency : file.dependency()) {
      bool present = false;
      for (const auto &existing : found->dependency())
        if (existing == dependency)
          present = true;
      if (!present)
        found->add_dependency(dependency);
    }
    for (const auto &message : file.message_type()) {
      bool present = false;
      for (const auto &existing : found->message_type()) {
        if (existing.name() != message.name())
          continue;
        if (encode(existing) != encode(message))
          throw invalid_argument("Conflicting Protobuf Message: " + message.name());
        present = true;
      }
      if (!present)
        *found->add_message_type() = message;
    }
    for (const auto &enumeration : file.enum_type()) {
      bool present = false;
      for (const auto &existing : found->enum_type()) {
        if (existing.name() != enumeration.name())
          continue;
        if (encode(existing) != encode(enumeration)) {
          throw invalid_argument("Conflicting Protobuf Enum: " + enumeration.name());
        }
        present = true;
      }
      if (!present)
        *found->add_enum_type() = enumeration;
    }
  }
}

namespace {
void normalize(DescriptorProto &message) {
  for (auto &field : *message.mutable_field())
    field.clear_json_name();
  for (auto &field : *message.mutable_extension())
    field.clear_json_name();
  for (auto &nested : *message.mutable_nested_type())
    normalize(nested);
}
} // namespace

FileDescriptorSet types(const Descriptor *descriptor) {
  set<string> seen;
  map<string, FileDescriptorProto> files;
  map<string, set<string>> selected;
  auto select = [&](const auto *node) {
    const string name(node->file()->name());
    if (!files.contains(name))
      node->file()->CopyTo(&files[name]);
    selected[name].insert(string(node->name()));
  };
  std::function<void(const Descriptor *)> visit;
  std::function<void(const Descriptor *)> fields;
  fields = [&](const auto *message) {
    for (int index = 0; index < message->field_count() + message->extension_count(); ++index) {
      const auto *field = index < message->field_count() ? message->field(index) : message->extension(index - message->field_count());
      if (field->message_type())
        visit(field->message_type());
      else if (field->enum_type()) {
        if (field->enum_type()->containing_type())
          visit(field->enum_type()->containing_type());
        else
          select(field->enum_type());
      }
      if (field->is_extension())
        visit(field->containing_type());
    }
    for (int index = 0; index < message->nested_type_count(); ++index)
      fields(message->nested_type(index));
  };
  visit = [&](const auto *message) {
    while (message->containing_type())
      message = message->containing_type();
    if (!seen.insert(string(message->full_name())).second)
      return;
    select(message);
    fields(message);
  };
  visit(descriptor);
  FileDescriptorSet result;
  for (const auto &[name, source] : files) {
    auto *file = result.add_file();
    *file = source;
    file->clear_message_type();
    file->clear_enum_type();
    for (const auto &message : source.message_type()) {
      if (selected[name].contains(message.name()))
        *file->add_message_type() = message;
    }
    for (const auto &enumeration : source.enum_type()) {
      if (selected[name].contains(enumeration.name()))
        *file->add_enum_type() = enumeration;
    }
    auto order = [](const auto &left, const auto &right) {
      return left.name() < right.name();
    };
    std::sort(file->mutable_message_type()->begin(), file->mutable_message_type()->end(), order);
    std::sort(file->mutable_enum_type()->begin(), file->mutable_enum_type()->end(), order);
    file->clear_service();
    file->clear_extension();
    file->clear_source_code_info();
    file->clear_options();
    file->clear_public_dependency();
    file->clear_weak_dependency();
    file->clear_dependency();
    set<string> dependencies;
    for (const auto &dependency : source.dependency())
      if (files.contains(dependency))
        dependencies.insert(dependency);
    for (const auto &dependency : dependencies)
      file->add_dependency(dependency);
    for (auto &message : *file->mutable_message_type())
      normalize(message);
  }
  return result;
}
} // namespace harmos::plugin::detail

namespace harmos::plugin {
namespace wire = ::harmos::model::v1;
Plugin::Plugin(string id, string version) : id_(std::move(id)), version_(std::move(version)) {}

Plugin &Plugin::on_stop(std::function<void()> cleanup) {
  cleanup_.push_back(std::move(cleanup));
  return *this;
}

Plugin &Plugin::setting(wire::SettingDeclaration declaration) {
  if (declaration.name().empty() || !declaration.has_default_() || declaration.default_().value_case() == wire::SettingValue::VALUE_NOT_SET)
    throw invalid_argument("Declare Setting " + declaration.name() + ": Name and Typed Default Are Required");
  for (const auto &existing : settings_.values())
    if (existing.name() == declaration.name())
      throw invalid_argument("Declare Setting " + declaration.name() + ": Duplicate Name");
  *settings_.add_values() = std::move(declaration);
  return *this;
}

wire::Declarations Plugin::declarations() const {
  wire::Declarations declarations;
  *declarations.mutable_types() = messages_;
  set<string> operations;
  set<string> requirements;
  set<string> streams;
  set<string> sections;
  if (id_.empty() || version_.empty())
    throw invalid_argument("Plugin Identity Is Required");
  for (const auto &operation : operations_) {
    if (!operations.insert(operation.contract.id()).second || !operation.handler) {
      throw invalid_argument("Duplicate or Unhandled Operation");
    }
    const auto visibility = operation.contract.visibility();
    if (visibility != wire::VISIBILITY_PRIVATE && visibility != wire::VISIBILITY_PROTECTED && visibility != wire::VISIBILITY_PUBLIC)
      throw invalid_argument("Register " + operation.contract.id() + ": Explicit Visibility Is Required");
    if (visibility == wire::VISIBILITY_PRIVATE)
      continue;
    *declarations.add_operations() = operation.contract;
    detail::merge(*declarations.mutable_types(), operation.types);
  }
  for (const auto &[requirement, types] : requirements_) {
    if (!requirements.insert(requirement.id()).second)
      throw invalid_argument("Duplicate Requirement");
    *declarations.add_requires_() = requirement;
    detail::merge(*declarations.mutable_types(), types);
  }
  for (const auto &stream : streams_) {
    if (!streams.insert(stream.declaration.id()).second)
      throw invalid_argument("Duplicate Stream");
    bool interval = stream.declaration.has_interval();
    bool setting = !stream.declaration.interval_setting().empty();
    if (bool(stream.observer) != (interval || setting))
      throw invalid_argument("Observer Requires an Interval");
    if (!stream.observer && !stream.producer)
      throw invalid_argument("Stream Requires a Producer");
    if (interval) {
      const auto &duration = stream.declaration.interval();
      if (duration.seconds() < 0 || duration.seconds() > 315576000000LL || duration.nanos() < 0 || duration.nanos() >= 1000000000 ||
          (duration.seconds() == 0 && duration.nanos() == 0))
        throw invalid_argument("Invalid Observer Interval");
    }
    *declarations.add_streams() = stream.declaration;
    detail::merge(*declarations.mutable_types(), stream.types);
  }
  for (const auto &[section, types] : sections_) {
    if (!sections.insert(section.type()).second)
      throw invalid_argument("Duplicate Section");
    *declarations.add_sections() = section;
    detail::merge(*declarations.mutable_types(), types);
  }
  if (!settings_.values().empty()) {
    const string type(wire::SettingsDeclarations::descriptor()->full_name());
    if (!sections.insert(type).second)
      throw invalid_argument("Declare Settings: Duplicate Section");
    auto *section = declarations.add_sections();
    section->set_type(type);
    section->set_payload(detail::encode(settings_));
    detail::merge(*declarations.mutable_types(), detail::types(wire::SettingsDeclarations::descriptor()));
  }
  declarations.set_digest(detail::sha256(detail::encode(declarations)));
  return declarations;
}
} // namespace harmos::plugin

/* === Private Functions === */

/* === Entry Point === */
