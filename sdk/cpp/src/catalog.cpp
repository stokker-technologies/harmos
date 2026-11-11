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

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

namespace harmos::detail {
std::string encode(const google::protobuf::Message& message) {
    std::string bytes;
    google::protobuf::io::StringOutputStream stream(&bytes);
    {
        google::protobuf::io::CodedOutputStream output(&stream);
        output.SetSerializationDeterministic(true);
        if (!message.SerializeToCodedStream(&output)) throw std::invalid_argument("Protobuf Encoding Failed");
    }
    return bytes;
}

std::string sha256(const std::string& bytes) {
    unsigned char digest[SHA256_DIGEST_LENGTH];
    SHA256(reinterpret_cast<const unsigned char*>(bytes.data()), bytes.size(), digest);
    return {reinterpret_cast<const char*>(digest), sizeof(digest)};
}

void merge(google::protobuf::FileDescriptorSet& target, const google::protobuf::FileDescriptorSet& source) {
    for (const auto& file : source.file()) {
        google::protobuf::FileDescriptorProto* found = nullptr;
        for (auto& existing : *target.mutable_file()) {
            if (existing.name() != file.name()) continue;
            found = &existing;
        }
        if (!found) { *target.add_file() = file; continue; }
        if (found->package() != file.package() || found->syntax() != file.syntax()) {
            throw std::invalid_argument("Conflicting Protobuf File: " + file.name());
        }
        for (const auto& dependency : file.dependency()) {
            bool present = false;
            for (const auto& existing : found->dependency()) if (existing == dependency) present = true;
            if (!present) found->add_dependency(dependency);
        }
        for (const auto& message : file.message_type()) {
            bool present = false;
            for (const auto& existing : found->message_type()) {
                if (existing.name() != message.name()) continue;
                if (encode(existing) != encode(message)) throw std::invalid_argument("Conflicting Protobuf Message: " + message.name());
                present = true;
            }
            if (!present) *found->add_message_type() = message;
        }
        for (const auto& enumeration : file.enum_type()) {
            bool present = false;
            for (const auto& existing : found->enum_type()) {
                if (existing.name() != enumeration.name()) continue;
                if (encode(existing) != encode(enumeration)) {
                    throw std::invalid_argument("Conflicting Protobuf Enum: " + enumeration.name());
                }
                present = true;
            }
            if (!present) *found->add_enum_type() = enumeration;
        }
    }
}

namespace {
void normalize(google::protobuf::DescriptorProto& message) {
    for (auto& field : *message.mutable_field()) field.clear_json_name();
    for (auto& field : *message.mutable_extension()) field.clear_json_name();
    for (auto& nested : *message.mutable_nested_type()) normalize(nested);
}
}

google::protobuf::FileDescriptorSet types(const google::protobuf::Descriptor* descriptor) {
    std::set<std::string> seen;
    std::map<std::string, google::protobuf::FileDescriptorProto> files;
    std::map<std::string, std::set<std::string>> selected;
    auto select = [&](const auto* node) {
        const std::string name(node->file()->name());
        if (!files.contains(name)) node->file()->CopyTo(&files[name]);
        selected[name].insert(std::string(node->name()));
    };
    std::function<void(const google::protobuf::Descriptor*)> visit;
    std::function<void(const google::protobuf::Descriptor*)> fields;
    fields = [&](const auto* message) {
        for (int index = 0; index < message->field_count() + message->extension_count(); ++index) {
            const auto* field = index < message->field_count() ? message->field(index)
                : message->extension(index - message->field_count());
            if (field->message_type()) visit(field->message_type());
            else if (field->enum_type()) {
                if (field->enum_type()->containing_type()) visit(field->enum_type()->containing_type());
                else select(field->enum_type());
            }
            if (field->is_extension()) visit(field->containing_type());
        }
        for (int index = 0; index < message->nested_type_count(); ++index) fields(message->nested_type(index));
    };
    visit = [&](const auto* message) {
        while (message->containing_type()) message = message->containing_type();
        if (!seen.insert(std::string(message->full_name())).second) return;
        select(message);
        fields(message);
    };
    visit(descriptor);
    google::protobuf::FileDescriptorSet result;
    for (const auto& [name, source] : files) {
        auto* file = result.add_file();
        *file = source;
        file->clear_message_type();
        file->clear_enum_type();
        for (const auto& message : source.message_type()) {
            if (selected[name].contains(message.name())) *file->add_message_type() = message;
        }
        for (const auto& enumeration : source.enum_type()) {
            if (selected[name].contains(enumeration.name())) *file->add_enum_type() = enumeration;
        }
        auto order = [](const auto& left, const auto& right) { return left.name() < right.name(); };
        std::sort(file->mutable_message_type()->begin(), file->mutable_message_type()->end(), order);
        std::sort(file->mutable_enum_type()->begin(), file->mutable_enum_type()->end(), order);
        file->clear_service();
        file->clear_extension();
        file->clear_source_code_info();
        file->clear_options();
        file->clear_public_dependency();
        file->clear_weak_dependency();
        file->clear_dependency();
        std::set<std::string> dependencies;
        for (const auto& dependency : source.dependency()) if (files.contains(dependency)) dependencies.insert(dependency);
        for (const auto& dependency : dependencies) file->add_dependency(dependency);
        for (auto& message : *file->mutable_message_type()) normalize(message);
    }
    return result;
}
}

namespace harmos {
Plugin::Plugin(std::string id, std::string version) : id_(std::move(id)), version_(std::move(version)) {}

Plugin& Plugin::on_stop(std::function<void()> cleanup) {
    cleanup_.push_back(std::move(cleanup));
    return *this;
}

wire::Catalog Plugin::catalog() const {
    wire::Catalog catalog;
    std::set<std::string> operations;
    std::set<std::string> requirements;
    std::set<std::string> streams;
    std::set<std::string> sections;
    if (id_.empty() || version_.empty()) throw std::invalid_argument("Plugin Identity Is Required");
    for (const auto& operation : operations_) {
        if (!operations.insert(operation.contract.id()).second || !operation.handler) {
            throw std::invalid_argument("Duplicate or Unhandled Operation");
        }
        *catalog.add_operations() = operation.contract;
        detail::merge(*catalog.mutable_types(), operation.types);
    }
    for (const auto& [requirement, types] : requirements_) {
        if (!requirements.insert(requirement.id()).second) throw std::invalid_argument("Duplicate Requirement");
        *catalog.add_requires_() = requirement;
        detail::merge(*catalog.mutable_types(), types);
    }
    for (const auto& stream : streams_) {
        if (!streams.insert(stream.declaration.id()).second) throw std::invalid_argument("Duplicate Stream");
        bool interval = stream.declaration.has_interval();
        bool setting = !stream.declaration.interval_setting().empty();
        if (bool(stream.observer) != (interval || setting)) throw std::invalid_argument("Observer Requires an Interval");
        if (!stream.observer && !stream.producer) throw std::invalid_argument("Stream Requires a Producer");
        if (interval) {
            const auto& duration = stream.declaration.interval();
            if (duration.seconds() < 0 || duration.seconds() > 315576000000LL || duration.nanos() < 0 || duration.nanos() >= 1000000000
                || (duration.seconds() == 0 && duration.nanos() == 0)) throw std::invalid_argument("Invalid Observer Interval");
        }
        *catalog.add_streams() = stream.declaration;
        detail::merge(*catalog.mutable_types(), stream.types);
    }
    for (const auto& [section, types] : sections_) {
        if (!sections.insert(section.type()).second) throw std::invalid_argument("Duplicate Section");
        *catalog.add_sections() = section;
        detail::merge(*catalog.mutable_types(), types);
    }
    catalog.set_digest(detail::sha256(detail::encode(catalog)));
    return catalog;
}
}

/* === Private Functions === */

/* === Entry Point === */
