// SPDX-License-Identifier: Apache-2.0

using System.Security.Cryptography;
using System.Text;
using Google.Protobuf;
using Google.Protobuf.Reflection;

namespace Harmos.Sdk;

internal static class Descriptors
{
  internal static ByteString Fingerprint(string id, MessageDescriptor request, MessageDescriptor response)
  {
    using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
    hash.AppendData(Encoding.UTF8.GetBytes(id));
    hash.AppendData([0]);
    hash.AppendData(Types(request).ToByteArray());
    hash.AppendData(Types(response).ToByteArray());
    return ByteString.CopyFrom(hash.GetHashAndReset());
  }

  internal static FileDescriptorSet Types(MessageDescriptor message)
  {
    var types = new FileDescriptorSet();
    var visited = new HashSet<string>();
    AddMessage(message, types, visited);
    return types;
  }

  private static FileDescriptorProto File(FileDescriptor descriptor, FileDescriptorSet types)
  {
    var file = types.File.FirstOrDefault(item => item.Name == descriptor.Name);
    if (file is not null)
      return file;
    file = new FileDescriptorProto { Name = descriptor.Name, Package = descriptor.Package, Syntax = descriptor.ToProto().Syntax };
    types.File.Add(file);
    return file;
  }

  private static void AddMessage(MessageDescriptor descriptor, FileDescriptorSet types, HashSet<string> visited)
  {
    while (descriptor.ContainingType is not null)
      descriptor = descriptor.ContainingType;
    if (!visited.Add(descriptor.FullName))
      return;
    var file = File(descriptor.File, types);
    var message = descriptor.ToProto();
    Normalize(message);
    file.MessageType.Add(message);
    AddFields(descriptor, types, visited);
  }

  private static void AddFields(MessageDescriptor message, FileDescriptorSet types, HashSet<string> visited)
  {
    var file = File(message.File, types);
    foreach (var field in message.Fields.InDeclarationOrder())
    {
      if (field.FieldType is FieldType.Message or FieldType.Group)
      {
        Dependency(file, field.MessageType.File);
        AddMessage(field.MessageType, types, visited);
      }
      else if (field.FieldType == FieldType.Enum)
      {
        Dependency(file, field.EnumType.File);
        if (field.EnumType.ContainingType is not null)
          AddMessage(field.EnumType.ContainingType, types, visited);
        else if (visited.Add(field.EnumType.FullName))
          File(field.EnumType.File, types).EnumType.Add(field.EnumType.ToProto());
      }
    }
    foreach (var nested in message.NestedTypes)
      AddFields(nested, types, visited);
  }

  private static void Dependency(FileDescriptorProto file, FileDescriptor dependency)
  {
    if (file.Name != dependency.Name && !file.Dependency.Contains(dependency.Name))
      file.Dependency.Add(dependency.Name);
  }

  private static void Normalize(DescriptorProto message)
  {
    foreach (var field in message.Field)
      field.ClearJsonName();
    foreach (var nested in message.NestedType)
      Normalize(nested);
  }

  internal static FileDescriptorSet Merge(FileDescriptorSet target, FileDescriptorSet source)
  {
    foreach (var file in source.File)
    {
      var existing = target.File.FirstOrDefault(item => item.Name == file.Name);
      if (existing is null)
        target.File.Add(file.Clone());
      else
      {
        if (existing.Package != file.Package || existing.Syntax != file.Syntax)
          throw new ArgumentException($"Conflicting Protobuf File: {file.Name}");
        foreach (var dependency in file.Dependency)
          if (!existing.Dependency.Contains(dependency))
            existing.Dependency.Add(dependency);
        foreach (var message in file.MessageType)
        {
          var found = existing.MessageType.FirstOrDefault(item => item.Name == message.Name);
          if (found is null)
            existing.MessageType.Add(message.Clone());
          else if (!found.Equals(message))
            throw new ArgumentException($"Conflicting Protobuf Message: {message.Name}");
        }
        foreach (var enumeration in file.EnumType)
        {
          var found = existing.EnumType.FirstOrDefault(item => item.Name == enumeration.Name);
          if (found is null)
            existing.EnumType.Add(enumeration.Clone());
          else if (!found.Equals(enumeration))
            throw new ArgumentException($"Conflicting Protobuf Enum: {enumeration.Name}");
        }
      }
    }
    return target;
  }
}
