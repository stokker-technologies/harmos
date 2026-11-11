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
    var files = new Dictionary<string, FileDescriptorProto>();
    var selected = new Dictionary<string, HashSet<string>>();
    var visited = new HashSet<string>();

    void Select(IDescriptor descriptor)
    {
      var file = descriptor.File;
      if (!files.ContainsKey(file.Name))
      {
        files.Add(file.Name, file.ToProto());
        selected.Add(file.Name, []);
      }
      selected[file.Name].Add(descriptor.Name);
    }

    void Visit(MessageDescriptor descriptor)
    {
      while (descriptor.ContainingType is not null)
        descriptor = descriptor.ContainingType;
      if (!visited.Add(descriptor.FullName))
        return;
      Select(descriptor);
      Fields(descriptor);
    }

    void Fields(MessageDescriptor descriptor)
    {
      foreach (var field in descriptor.Fields.InDeclarationOrder().Concat(descriptor.Extensions.UnorderedExtensions))
      {
        if (field.FieldType is FieldType.Message or FieldType.Group)
          Visit(field.MessageType);
        else if (field.FieldType == FieldType.Enum)
        {
          if (field.EnumType.ContainingType is not null)
            Visit(field.EnumType.ContainingType);
          else
            Select(field.EnumType);
        }
        if (field.IsExtension)
          Visit(field.ExtendeeType);
      }
      foreach (var nested in descriptor.NestedTypes)
        Fields(nested);
    }

    Visit(message);
    var types = new FileDescriptorSet();
    foreach (var (name, source) in files.OrderBy(item => item.Key, Utf8Order))
    {
      var file = source.Clone();
      file.MessageType.Clear();
      file.MessageType.Add(source.MessageType.Where(item => selected[name].Contains(item.Name)).OrderBy(item => item.Name, Utf8Order));
      file.EnumType.Clear();
      file.EnumType.Add(source.EnumType.Where(item => selected[name].Contains(item.Name)).OrderBy(item => item.Name, Utf8Order));
      file.Service.Clear();
      file.Extension.Clear();
      file.SourceCodeInfo = null;
      file.Options = null;
      file.PublicDependency.Clear();
      file.WeakDependency.Clear();
      file.Dependency.Clear();
      file.Dependency.Add(source.Dependency.Where(files.ContainsKey).Distinct().OrderBy(name => name, Utf8Order));
      foreach (var definition in file.MessageType)
        Normalize(definition);
      types.File.Add(file);
    }
    return types;
  }

  private static readonly IComparer<string> Utf8Order = Comparer<string>.Create((left, right) =>
      Encoding.UTF8.GetBytes(left).AsSpan().SequenceCompareTo(Encoding.UTF8.GetBytes(right)));

  private static void Normalize(DescriptorProto message)
  {
    foreach (var field in message.Field.Concat(message.Extension))
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
