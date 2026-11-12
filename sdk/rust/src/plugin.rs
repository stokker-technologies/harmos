/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust Plugin SDK

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

// Third Party Imports

// Local Imports
use crate::{Error, Message, Observer, Operation, OperationRequest, Result, Stream, message::merge_types, stream::StreamDefinition};
use harmos_protocol::v1;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Plugin {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) operations: Vec<Operation>,
    pub(crate) streams: Vec<StreamDefinition>,
    pub(crate) observers: Vec<Observer>,
    pub(crate) requirements: Vec<Required>,
    sections: Vec<(v1::Section, prost_types::FileDescriptorSet)>,
    messages: Vec<(String, prost_types::FileDescriptorSet)>,
    pub(crate) cleanup: Vec<Box<dyn Fn() -> Result<()> + Send + Sync>>,
    pub(crate) remote: bool,
}

pub(crate) struct Required {
    pub(crate) id: String,
    pub(crate) watch: bool,
    pub(crate) request: String,
    pub(crate) response: String,
    pub(crate) types: prost_types::FileDescriptorSet,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Plugin {
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            operations: Vec::new(),
            streams: Vec::new(),
            observers: Vec::new(),
            requirements: Vec::new(),
            sections: Vec::new(),
            messages: Vec::new(),
            cleanup: Vec::new(),
            remote: false,
        }
    }

    pub fn operation(mut self, operation: Operation) -> Self {
        self.operations.push(operation);
        self
    }
    pub fn stream<T: Message>(mut self, stream: Stream<T>) -> Self {
        self.streams.push(stream.definition);
        self
    }
    pub fn observer(mut self, observer: Observer) -> Self {
        self.observers.push(observer);
        self
    }
    /// Register a Payload Descriptor Without Adding an Operation, Stream or Section.
    pub fn message<T: Message>(mut self) -> Self {
        self.messages.push((T::name(), T::types()));
        self
    }

    pub fn section<T: Message>(mut self, value: T) -> Self {
        self.sections.push((
            v1::Section {
                r#type: T::name(),
                payload: value.encode_to_vec(),
            },
            T::types(),
        ));
        self
    }

    pub fn requires<Q: OperationRequest>(mut self) -> Self {
        let mut types = Q::types();
        crate::message::merge_descriptor_types(&mut types, Q::Response::types());
        self.requirements.push(Required {
            id: Q::ID.into(),
            request: Q::name(),
            response: Q::Response::name(),
            watch: Q::WATCH,
            types,
        });
        self
    }

    /// Stop hooks complete in registration order before the RPC replies.
    pub fn on_stop(mut self, cleanup: impl Fn() -> Result<()> + Send + Sync + 'static) -> Self {
        self.cleanup.push(Box::new(cleanup));
        self
    }

    /// Remote processes remain available after Stop. HARMOS_ADDRESS controls binding in either mode.
    pub fn remote(mut self) -> Self {
        self.remote = true;
        self
    }

    pub fn catalog(&self) -> Result<v1::Catalog> {
        let id_valid = !self.id.is_empty()
            && self.id.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'_');
        if !id_valid {
            return Err(Error::Invalid("Plugin Identity Must Be Lowercase with Dots or Underscores".into()));
        }
        semver::Version::parse(&self.version).map_err(|error| Error::Invalid(format!("Invalid Plugin Version: {error}")))?;
        let mut catalog = v1::Catalog::default();
        let mut types = prost_types::FileDescriptorSet::default();
        for (_, definitions) in &self.messages {
            merge_types(&mut types, definitions.clone())?;
        }
        let mut ids = HashSet::new();
        for operation in &self.operations {
            if operation.contract.id.is_empty() || !ids.insert(&operation.contract.id) {
                return Err(Error::Invalid(format!("Duplicate or Empty Operation: {}", operation.contract.id)));
            }
            if operation.contract.visibility != v1::Visibility::Private as i32
                && operation.contract.visibility != v1::Visibility::Protected as i32
                && operation.contract.visibility != v1::Visibility::Public as i32
            {
                return Err(Error::Invalid(format!("Invalid Visibility: {}", operation.contract.id)));
            }
            let mut contract = operation.contract.clone();
            contract.fingerprint =
                harmos_protocol::schema::fingerprint(&contract.id, &operation.types, &contract.request, &contract.response)
                    .map_err(|error| Error::Invalid(error.to_string()))?;
            if contract.visibility == v1::Visibility::Private as i32 {
                continue;
            }
            catalog.operations.push(contract);
            merge_types(&mut types, operation.types.clone())?;
        }
        let mut requires = HashSet::new();
        for requirement in &self.requirements {
            if self
                .operations
                .iter()
                .any(|operation| operation.contract.id == requirement.id && operation.contract.visibility == v1::Visibility::Private as i32)
            {
                return Err(Error::Invalid(format!("Require {}: Private Calls Are Local", requirement.id)));
            }
            if !requires.insert(&requirement.id) {
                return Err(Error::Invalid(format!("Duplicate Requirement: {}", requirement.id)));
            }
            let fingerprint = harmos_protocol::schema::host_fingerprint(
                &requirement.id,
                &v1::Protobuf {
                    descriptors: prost::Message::encode_to_vec(&requirement.types),
                    request: requirement.request.clone(),
                    response: requirement.response.clone(),
                    input: None,
                    output: requirement.watch.then(|| requirement.response.clone()),
                },
            )
            .map_err(|error| Error::Invalid(error.to_string()))?;
            catalog.requires.push(v1::Requirement { id: requirement.id.clone(), fingerprint });
            merge_types(&mut types, requirement.types.clone())?;
        }
        let mut streams = HashSet::new();
        for stream in &self.streams {
            if stream.declaration.id.is_empty() || !streams.insert(&stream.declaration.id) {
                return Err(Error::Invalid(format!("Duplicate or Empty Stream: {}", stream.declaration.id)));
            }
            let mut declaration = stream.declaration.clone();
            let observers = self.observers.iter().filter(|observer| observer.stream == declaration.id).collect::<Vec<_>>();
            if observers.len() > 1 || (!observers.is_empty() && stream.producer.is_some()) {
                return Err(Error::Invalid(format!("Multiple Stream Producers: {}", declaration.id)));
            }
            if let Some(observer) = observers.first() {
                let positive_interval = observer.interval.is_some_and(|interval| interval.seconds > 0 || interval.nanos > 0);
                let invalid_default = observer.interval.is_some() && !positive_interval;
                let missing_interval = observer.interval.is_none() && observer.interval_setting.is_empty();
                if invalid_default || missing_interval {
                    return Err(Error::Invalid(format!("Positive Observer Interval Required: {}", declaration.id)));
                }
                declaration.interval = observer.interval;
                declaration.interval_setting = observer.interval_setting.clone();
            }
            catalog.streams.push(declaration);
            merge_types(&mut types, stream.types.clone())?;
        }
        for observer in &self.observers {
            if !streams.contains(&observer.stream) {
                return Err(Error::Invalid(format!("Undeclared Observer Stream: {}", observer.stream)));
            }
        }
        let mut sections = HashSet::new();
        for (section, definitions) in &self.sections {
            if !sections.insert(&section.r#type) {
                return Err(Error::Invalid(format!("Duplicate Section: {}", section.r#type)));
            }
            catalog.sections.push(section.clone());
            merge_types(&mut types, definitions.clone())?;
        }
        let pool = prost_reflect::DescriptorPool::from_file_descriptor_set(types.clone())
            .map_err(|error| Error::Invalid(format!("Invalid Protobuf Definitions: {error}")))?;
        let names = catalog
            .operations
            .iter()
            .flat_map(|operation| [&operation.request, &operation.response])
            .chain(catalog.streams.iter().map(|stream| &stream.row))
            .chain(catalog.sections.iter().map(|section| &section.r#type))
            .chain(self.messages.iter().map(|(name, _)| name));
        for name in names {
            if pool.get_message_by_name(name).is_none() {
                return Err(Error::Invalid(format!("Missing Protobuf Message: {name}")));
            }
        }
        catalog.types = Some(types);
        catalog.digest = harmos_protocol::schema::catalog_digest(&catalog);
        Ok(catalog)
    }

    pub(crate) fn requirements(catalog: &v1::Catalog) -> Arc<HashMap<String, Vec<u8>>> {
        Arc::new(catalog.requires.iter().map(|requirement| (requirement.id.clone(), requirement.fingerprint.clone())).collect())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    async fn empty(_: &Context, _: ()) -> Result<()> {
        Ok(())
    }
    async fn observe(_: &Context) -> Result<()> {
        Ok(())
    }

    #[derive(crate::Message)]
    struct Settings {
        interval_ms: u64,
    }

    async fn not_called(_: &Context, _: ()) -> Result<()> {
        panic!("Catalog Inspection Entered A Handler");
    }

    #[test]
    fn catalog_describes_values_without_running_handlers_or_cleanup() {
        let plugin = Plugin::new("catalog_fixture", "1.0.0")
            .operation(Operation::new("catalog.read", not_called).visibility(harmos_protocol::v1::Visibility::Protected))
            .section(Settings { interval_ms: 250 })
            .on_stop(|| panic!("Catalog Inspection Entered Cleanup"));
        let catalog = plugin.catalog().unwrap();
        assert_eq!(catalog.operations.len(), 1);
        assert_eq!(catalog.operations[0].id, "catalog.read");
        assert_eq!(catalog.sections.len(), 1);
        let settings = <Settings as prost::Message>::decode(catalog.sections[0].payload.as_slice()).unwrap();
        assert_eq!(settings.interval_ms, 250);
    }

    #[test]
    fn duplicate_operations_are_rejected_at_catalog_construction() {
        let plugin = Plugin::new("test", "1.0.0")
            .operation(Operation::new("test.duplicate", empty).visibility(harmos_protocol::v1::Visibility::Protected))
            .operation(Operation::new("test.duplicate", empty).visibility(harmos_protocol::v1::Visibility::Protected));
        assert!(plugin.catalog().is_err());
    }

    #[test]
    fn observer_must_own_one_declared_stream() {
        let stream = Stream::<()>::new("test.samples");
        let observer = Observer::new(&stream, observe).every(prost_types::Duration { seconds: 1, nanos: 0 });
        assert!(Plugin::new("test", "1.0.0").observer(observer).catalog().is_err());
        let first = Observer::new(&stream, observe).every(prost_types::Duration { seconds: 1, nanos: 0 });
        let second = Observer::new(&stream, observe).every(prost_types::Duration { seconds: 1, nanos: 0 });
        assert!(Plugin::new("test", "1.0.0").stream(stream).observer(first).observer(second).catalog().is_err());
    }
}
