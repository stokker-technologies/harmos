/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Stage Deterministic Standard Protobuf Sources And Producer Packaging Metadata.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
// Third Party Imports
use heck::{ToShoutySnakeCase, ToUpperCamelCase};
use prost_reflect::{DescriptorPool, MessageDescriptor, prost::Message as _, prost_types::FileDescriptorSet};
// Local Imports
use super::{Error, Settings, identifier, python_version};
use crate::proto::Artifacts;
use harmos_domain::operations::Contract;

/* === Constants === */
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_OUTPUT_FILES: usize = 16_384;
const HEADER: &str = "# ***\n#\n# Harmos\n#\n# SPDX-License-Identifier: Apache-2.0\n#\n# ***\n\n";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
pub(super) fn export(
    artifacts: &Artifacts, contracts: &[Contract], settings: &Settings, output: &Path, protoc: &Path,
) -> Result<PathBuf, Error> {
    let parent = output.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new().prefix(".harmos-python-").tempdir_in(parent)?;
    let source = staging.path().join("src");
    let module = source.join(&settings.module);
    let wire = module.join("_wire");
    fs::create_dir_all(&wire)?;

    // Prefix File Identities Without Changing Protobuf Type Names Or RPC Paths.
    let pool = DescriptorPool::decode(artifacts.descriptors())?;
    let mut set = FileDescriptorSet { file: pool.files().map(|file| file.file_descriptor_proto().clone()).collect() };
    let prefix = format!("{}/_wire/", settings.module);
    for file in &mut set.file {
        validate_file(file)?;
        file.name = Some(format!("{prefix}{}", file.name()));
        for dependency in &mut file.dependency {
            *dependency = format!("{prefix}{dependency}");
        }
    }
    let compiler_input = staging.path().join("compiler.bin");
    fs::write(&compiler_input, set.encode_to_vec())?;
    let compiler = Command::new(protoc)
        .arg(format!("--descriptor_set_in={}", compiler_input.display()))
        .arg(format!("--python_out={}", source.display()))
        .arg(format!("--pyi_out={}", source.display()))
        .args(set.file.iter().map(|file| file.name()))
        .output()?;
    if !compiler.status.success() {
        return Err(Error::Compiler(String::from_utf8_lossy(&compiler.stderr).chars().take(4096).collect()));
    }
    fs::remove_file(&compiler_input)?;

    // Isolate Runtime Descriptors And Retain Standard Generated Typing Stubs.
    let runtime = isolate(&source, &settings.module)?;
    fs::write(wire.join("_pool.py"), include_str!("pool.py"))?;
    fs::write(module.join("operation.py"), include_str!("operation.py"))?;
    fs::write(module.join("dynamic.py"), include_str!("dynamic.py"))?;
    fs::write(module.join("py.typed"), "")?;
    fs::write(module.join("contracts.json"), artifacts.contracts())?;
    fs::write(module.join("descriptors.bin"), artifacts.descriptors())?;
    let (bindings, stub) = bindings(&pool, contracts, &settings.module)?;
    fs::write(module.join("__init__.py"), &bindings)?;
    fs::write(module.join("__init__.pyi"), &stub)?;
    package_metadata(staging.path(), settings, &runtime)?;
    bounded_files(staging.path())?;

    // Reserve The Destination Atomically; Existing Files And Directories Are Never Replaced.
    match fs::create_dir(output) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(Error::Exists(output.to_owned()));
        }
        Err(error) => return Err(error.into()),
    }
    let result = (|| {
        for entry in fs::read_dir(staging.path())? {
            let entry = entry?;
            fs::rename(entry.path(), output.join(entry.file_name()))?;
        }
        Ok::<_, Error>(output.to_owned())
    })();
    if let Err(error) = result {
        if let Err(source) = fs::remove_dir_all(output) {
            return Err(Error::Cleanup { cause: error.to_string(), source });
        }
        return Err(error);
    }
    Ok(output.to_owned())
}

fn validate_file(file: &prost_reflect::prost_types::FileDescriptorProto) -> Result<(), Error> {
    let name = file.name();
    let stem = name.strip_suffix(".proto").ok_or_else(|| Error::Unsupported("protobuf file suffix".into()))?;
    if stem.split('/').count() > 64 || !stem.split('/').all(identifier) || name.starts_with("google/protobuf/") {
        return Err(Error::Unsupported(format!("protobuf module path {name}")));
    }
    if !file.enum_type.is_empty() || !file.extension.is_empty() || file.message_type.iter().any(unsupported_message) {
        return Err(Error::Unsupported(format!("external enum/extension declarations in {name}")));
    }
    Ok(())
}

fn unsupported_message(message: &prost_reflect::prost_types::DescriptorProto) -> bool {
    !message.enum_type.is_empty() || !message.extension.is_empty() || message.nested_type.iter().any(unsupported_message)
}

fn bounded_files(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut directories = vec![root.to_owned()];
    let mut files = Vec::new();
    let mut bytes = 0usize;
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if files.len() + directories.len() >= MAX_OUTPUT_FILES {
                return Err(Error::Limit);
            }
            if kind.is_dir() {
                directories.push(entry.path());
            } else if kind.is_file() {
                let length = usize::try_from(entry.metadata()?.len()).map_err(|_| Error::Limit)?;
                bytes = bytes.checked_add(length).filter(|bytes| *bytes <= MAX_OUTPUT_BYTES).ok_or(Error::Limit)?;
                files.push(entry.path());
            } else {
                return Err(Error::Unsupported("compiler created a special file".into()));
            }
        }
    }
    files.sort_unstable();
    Ok(files)
}

fn isolate(source: &Path, module: &str) -> Result<String, Error> {
    let files = bounded_files(source)?;
    let mut runtime = None;
    let mut packages = BTreeSet::new();
    for path in files {
        if path.extension().is_none_or(|extension| extension != "py") {
            continue;
        }
        let mut text = fs::read_to_string(&path)?;
        let version = runtime_version(&text)?;
        if runtime.as_ref().is_some_and(|previous| previous != &version) {
            return Err(Error::Unsupported("compiler emitted mixed runtime versions".into()));
        }
        runtime = Some(version);
        for required in ["_descriptor_pool.Default()", "_builder.BuildTopDescriptorsAndMessages(", "_sym_db = _symbol_database.Default()"] {
            if text.matches(required).count() != 1 {
                return Err(Error::Unsupported("unrecognized protoc Python output".into()));
            }
        }
        text = text
            .replace("from google.protobuf import descriptor_pool as _descriptor_pool\n", "")
            .replace("from google.protobuf import symbol_database as _symbol_database\n", "")
            .replace("_sym_db = _symbol_database.Default()\n", "")
            .replace("_descriptor_pool.Default()", "_pool.POOL")
            .replace("_builder.BuildTopDescriptorsAndMessages(", "_pool.build(");
        let import = format!("from {module}._wire import _pool\n");
        let position = text
            .find("from google.protobuf import descriptor as _descriptor\n")
            .ok_or_else(|| Error::Unsupported("unrecognized protoc imports".into()))?;
        text.insert_str(position, &import);
        fs::write(&path, text)?;
        let mut directory = path.parent().expect("generated file has parent");
        while directory != source {
            packages.insert(directory.to_owned());
            directory = directory.parent().ok_or(Error::Limit)?;
        }
    }
    for directory in packages {
        fs::write(directory.join("__init__.py"), HEADER)?;
    }
    runtime.ok_or_else(|| Error::Unsupported("compiler produced no Python messages".into()))
}

fn runtime_version(text: &str) -> Result<String, Error> {
    let suffix = text
        .split_once("_runtime_version.ValidateProtobufRuntimeVersion(")
        .and_then(|(_, suffix)| suffix.split_once("_runtime_version.Domain.PUBLIC,"))
        .map(|(_, suffix)| suffix)
        .ok_or_else(|| Error::Unsupported("compiler has no public runtime version guard".into()))?;
    let mut parts = suffix.split(',');
    let mut version = Vec::new();
    for _ in 0..3 {
        let number = parts.next().ok_or(Error::Limit)?.trim();
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Error::Unsupported("compiler runtime version".into()));
        }
        version.push(number);
    }
    if !matches!(parts.next().map(str::trim), Some("''" | "\"\"")) {
        return Err(Error::Unsupported("non-stable protobuf compiler".into()));
    }
    Ok(version.join("."))
}

fn bindings(pool: &DescriptorPool, contracts: &[Contract], module: &str) -> Result<(String, String), Error> {
    let mut imports = BTreeMap::new();
    let mut body = String::new();
    let mut stub_body = String::new();
    let mut names = BTreeSet::from(["Operation".to_owned(), "OperationError".into(), "Invoker".into(), "ResponseStream".into()]);
    for contract in contracts {
        let grpc = contract.grpc().ok_or_else(|| Error::Unsupported("missing gRPC binding".into()))?;
        let service = pool
            .get_service_by_name(&format!("{}.{}", grpc.package, grpc.service))
            .ok_or_else(|| Error::Unsupported("missing service descriptor".into()))?;
        let method = service
            .methods()
            .find(|method| method.name() == grpc.method.as_ref())
            .ok_or_else(|| Error::Unsupported("missing method descriptor".into()))?;
        if method.is_client_streaming() != contract.input().is_some() || method.is_server_streaming() != contract.output().is_some() {
            return Err(Error::Unsupported("streaming descriptor differs from contract".into()));
        }
        let root = pool
            .get_message_by_name(&format!("{}.{}Messages.{}.Row0", grpc.package, grpc.service, grpc.method))
            .ok_or_else(|| Error::Unsupported("missing contract root descriptor".into()))?;
        let prefix = contract.operation().id.as_str().to_upper_camel_case();
        let constant = contract.operation().id.as_str().to_shouty_snake_case();
        if !identifier(&prefix) || !identifier(&constant) || !names.insert(constant.clone()) {
            return Err(Error::Collision(contract.operation().id.to_string()));
        }
        let mut directions = BTreeMap::new();
        for direction in ["request", "response", "input", "output"] {
            let Some(field) = root.get_field_by_name(direction) else {
                continue;
            };
            let prost_reflect::Kind::Message(message) = field.kind() else {
                return Err(Error::Unsupported("non-message operation direction".into()));
            };
            let expression = message_expression(&message, module, &mut imports)?;
            let name = format!("{prefix}{}", direction.to_upper_camel_case());
            if !names.insert(name.clone()) {
                return Err(Error::Collision(name));
            }
            writeln!(body, "{name} = {expression}").expect("String writer");
            writeln!(stub_body, "{name} = {expression}").expect("String writer");
            directions.insert(direction, name);
        }
        let request = directions.get("request").ok_or_else(|| Error::Unsupported("request direction".into()))?;
        let response = directions.get("response").ok_or_else(|| Error::Unsupported("response direction".into()))?;
        let input = directions.get("input");
        let output = directions.get("output");
        let mode = match (input.is_some(), output.is_some()) {
            (false, false) => "unary",
            (true, false) => "client_stream",
            (false, true) => "server_stream",
            (true, true) => "bidi",
        };
        let rpc_request = message_expression(&method.input(), module, &mut imports)?;
        let rpc_response = message_expression(&method.output(), module, &mut imports)?;
        let encoded = serde_json::to_string(contract)?;
        let fingerprint = crate::digest(encoded.as_bytes());
        let input_type = input.map_or("Message", String::as_str);
        let output_type = output.map_or("Message", String::as_str);
        writeln!(stub_body, "\n{constant}: Operation[{request}, {response}, {input_type}, {output_type}]\n").expect("String writer");
        writeln!(body, "\n{constant}: Operation[{request}, {response}, {input_type}, {output_type}] = Operation(\n    id={},\n    version={},\n    mode={},\n    codec=\"protobuf\",\n    request={request},\n    response={response},\n    input={},\n    output={},\n    grpc_path={},\n    rpc_request={rpc_request},\n    rpc_response={rpc_response},\n    contract={},\n    fingerprint={},\n)\n", quoted(contract.operation().id.as_str())?, contract.version(), quoted(mode)?, input.map_or("None", String::as_str), output.map_or("None", String::as_str), quoted(&grpc.path())?, quoted(&encoded)?, quoted(&fingerprint)?).expect("String writer");
    }
    let mut result = String::from(HEADER);
    result.push_str("\"\"\"Generated Operation Bindings From The Selected Producer Catalog\"\"\"\n\n# === Imports === #\n\n# Standard Library Imports\n\n# Third Party Imports\nfrom google.protobuf.message import Message\n\n# Local Imports\nfrom .operation import Invoker as Invoker, Operation as Operation, OperationError as OperationError, ResponseStream as ResponseStream\n");
    let mut stub = result.clone();
    for (name, alias) in &imports {
        writeln!(result, "import {name} as {alias}").expect("String writer");
        // RPC Envelopes Exist Only At Run Time; The Stub Imports What Its Annotations Name.
        if stub_body.contains(&format!("{alias}.")) {
            writeln!(stub, "import {name} as {alias}").expect("String writer");
        }
    }
    let sections = "\n# Type Checking Imports\n\n# === Public Exports === #\n\n# === Constants === #\n\n# === Types === #\n\n";
    result.push_str(sections);
    stub.push_str(sections);
    stub.push_str(&stub_body);
    result.push_str(&body);
    let footer = "# === Classes === #\n\n# === Public Functions === #\n\n# === Private Functions === #\n\n# === Entry Point === #\n";
    result.push_str(footer);
    stub.push_str(footer);
    Ok((result, stub))
}

fn message_expression(message: &MessageDescriptor, module: &str, imports: &mut BTreeMap<String, String>) -> Result<String, Error> {
    let file = message.parent_file();
    let stem = file.name().strip_suffix(".proto").ok_or_else(|| Error::Unsupported("message file suffix".into()))?;
    let name = format!("{module}._wire.{}_pb2", stem.replace('/', "."));
    let index = imports.len();
    let alias = imports.entry(name).or_insert_with(|| format!("_wire_{index}"));
    let local = message
        .full_name()
        .strip_prefix(&format!("{}.", file.package_name()))
        .ok_or_else(|| Error::Unsupported("message outside its package".into()))?;
    if !local.split('.').all(identifier) {
        return Err(Error::Unsupported("message Python identifier".into()));
    }
    Ok(format!("{alias}.{local}"))
}

fn package_metadata(root: &Path, settings: &Settings, runtime: &str) -> Result<(), Error> {
    let version = python_version(&settings.version)?;
    let data = format!(
        "[build-system]\nrequires = [\"setuptools>=77\"]\nbuild-backend = \"setuptools.build_meta\"\n\n[project]\nname = {}\nversion = {}\ndescription = {}\nlicense = {}\nrequires-python = \">=3.13\"\ndependencies = [{}]\n\n[tool.setuptools.packages.find]\nwhere = [\"src\"]\n\n[tool.setuptools.package-data]\n{} = [\"py.typed\", \"*.pyi\", \"contracts.json\", \"descriptors.bin\"]\n\"*\" = [\"*.pyi\"]\n",
        quoted(&settings.distribution)?,
        quoted(&version)?,
        quoted(&settings.description)?,
        quoted(&settings.license)?,
        quoted(&format!("protobuf>={runtime}"))?,
        quoted(&settings.module)?
    );
    fs::write(root.join("pyproject.toml"), data)?;
    Ok(())
}

fn quoted(value: &str) -> Result<String, Error> {
    Ok(serde_json::to_string(value)?)
}

/* === Tests === */
