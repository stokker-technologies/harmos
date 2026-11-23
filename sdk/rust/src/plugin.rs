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

include!("plugin/types.rs");
mod host;
mod registration;
include!("plugin/host_exports.rs");

/* === Re-Exports === */

pub use registration::{Contribution, Dependency, Descriptor, Registrar, Registration};

pub use crate::context::{Context, StateHandle, Subscription, SubscriptionItem, SubscriptionRow, Watching};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::native::Listener;

pub use crate::operation::Operation;
pub use crate::stream::{Collection, CollectionOutput, Observer, Publisher, Stream, Window};

/* === Imports === */

// Standard Library Imports
use std::{
    collections::{HashMap, HashSet},
    error,
    sync::Arc,
};

// Third Party Imports
use prost_reflect::DescriptorPool;
use prost_types::FileDescriptorSet;
use semver::Version;

// Local Imports
use crate::{Error, Result, message::merge_types, stream::StreamDefinition};
use harmos_protocol::model::Protobuf;
use harmos_protocol::model::Requirement;
use harmos_protocol::model::Section;
use harmos_protocol::model::StorageTransferSettings;
use harmos_protocol::model::Visibility;
use harmos_protocol::model::{Declarations, LaneDeclaration, lane_declaration};
use harmos_protocol::schema;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// A Plugin Lane Declaration; the Host Owns Admission.
#[derive(Clone, Debug)]
pub struct Lane {
    pub(crate) declaration: LaneDeclaration,
}

pub struct Plugin {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) operations: Vec<Operation>,
    lanes: Vec<Lane>,
    pub(crate) streams: Vec<StreamDefinition>,
    pub(crate) observers: Vec<Observer>,
    pub(crate) requirements: Vec<Required>,
    sections: Vec<(Section, FileDescriptorSet)>,
    messages: Vec<(String, FileDescriptorSet)>,
    pub(crate) cleanup: Vec<Box<dyn Fn() -> Result<()> + Send + Sync>>,
    pub(crate) remote: bool,
    pub(crate) transfer: StorageTransferSettings,
    pub(crate) failure_mapper: Option<crate::error::FailureMapper>,
}

pub(crate) struct Required {
    pub(crate) id: String,
    pub(crate) watch: bool,
    pub(crate) request: String,
    pub(crate) response: String,
    pub(crate) types: FileDescriptorSet,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Lane {
    pub fn serial(name: impl Into<String>) -> Self {
        Self {
            declaration: LaneDeclaration {
                name: name.into(),
                capacity: Some(lane_declaration::Capacity::Serial(true)),
            },
        }
    }

    pub fn concurrent(name: impl Into<String>, width: u32) -> Self {
        Self {
            declaration: LaneDeclaration {
                name: name.into(),
                capacity: Some(lane_declaration::Capacity::Concurrent(width)),
            },
        }
    }

    pub fn unbounded(name: impl Into<String>) -> Self {
        Self {
            declaration: LaneDeclaration {
                name: name.into(),
                capacity: Some(lane_declaration::Capacity::Unbounded(true)),
            },
        }
    }

    pub(crate) fn name(&self) -> &str {
        &self.declaration.name
    }
}

impl Plugin {
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            operations: Vec::new(),
            lanes: Vec::new(),
            streams: Vec::new(),
            observers: Vec::new(),
            requirements: Vec::new(),
            sections: Vec::new(),
            messages: Vec::new(),
            cleanup: Vec::new(),
            remote: false,
            transfer: Default::default(),
            failure_mapper: None,
        }
    }

    /// Map an Unstructured Handler Error Before the Permanent Fallback.
    pub fn failure_mapper(mut self, mapper: impl Fn(&(dyn error::Error + 'static)) -> Option<Error> + Send + Sync + 'static) -> Self {
        self.failure_mapper = Some(Arc::new(mapper));
        self
    }

    pub fn transfer(mut self, settings: StorageTransferSettings) -> Self {
        self.transfer = settings;
        self
    }

    pub fn register(mut self, register: impl FnOnce(&mut Registrar<'_>) -> Result<()>) -> Result<Self> {
        register(&mut Registrar { plugin: &mut self })?;
        Ok(self)
    }

    pub fn lane(mut self, lane: Lane) -> Self {
        self.lanes.push(lane);
        self
    }

    /// Stop hooks complete in registration order before the RPC replies.
    pub fn on_stop(mut self, cleanup: impl Fn() -> Result<()> + Send + Sync + 'static) -> Self {
        self.cleanup.push(Box::new(cleanup));
        self
    }

    /// Remote processes remain available after Stop.
    pub fn remote(mut self) -> Self {
        self.remote = true;
        self
    }

    pub fn declarations(&self) -> Result<Declarations> {
        let id_valid = !self.id.is_empty()
            && self.id.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'_');
        if !id_valid {
            return Err(Error::Invalid("Plugin Identity Must Be Lowercase with Dots or Underscores".into()));
        }
        Version::parse(&self.version).map_err(|error| Error::Invalid(format!("Invalid Plugin Version: {error}")))?;
        let mut declarations = Declarations {
            lanes: self.lanes.iter().map(|lane| lane.declaration.clone()).collect(),
            ..Default::default()
        };
        for operation in &self.operations {
            if operation.lane.is_some()
                && (operation.contract.id == format!("{}.initialize", self.id) || operation.contract.id == format!("{}.finalize", self.id))
            {
                return Err(Error::Invalid(format!(
                    "Declare Operation {} Lane `{}`: Remove the Lifecycle Lane Assignment",
                    operation.contract.id, operation.contract.lane
                )));
            }
            if let Some(lane) = &operation.lane
                && !self.lanes.iter().any(|declared| declared.declaration == lane.declaration)
            {
                return Err(Error::Invalid(format!(
                    "Declare Operation {} Lane `{}`: Declare This Lane in the Plugin",
                    operation.contract.id, operation.contract.lane
                )));
            }
        }
        let mut types = FileDescriptorSet::default();
        for (_, definitions) in &self.messages {
            merge_types(&mut types, definitions.clone())?;
        }
        let mut ids = HashSet::new();
        for operation in &self.operations {
            if operation.contract.id.is_empty() || !ids.insert(&operation.contract.id) {
                return Err(Error::Invalid(format!("Duplicate or Empty Operation: {}", operation.contract.id)));
            }
            if operation.contract.visibility != Visibility::Private as i32
                && operation.contract.visibility != Visibility::Protected as i32
                && operation.contract.visibility != Visibility::Public as i32
            {
                return Err(Error::Invalid(format!("Invalid Visibility: {}", operation.contract.id)));
            }
            let mut contract = operation.contract.clone();
            contract.fingerprint = schema::fingerprint(&contract.id, &operation.types, &contract.request, &contract.response)
                .map_err(|error| Error::Invalid(error.to_string()))?;
            schema::plugin_operation_identity(&contract.id).map_err(|error| Error::Invalid(error.to_string()))?;
            declarations.operations.push(contract);
            merge_types(&mut types, operation.types.clone())?;
        }
        let mut requires = HashSet::new();
        for requirement in &self.requirements {
            schema::operation_identity(&requirement.id).map_err(|error| Error::Invalid(error.to_string()))?;
            if !requires.insert(&requirement.id) {
                return Err(Error::Invalid(format!("Duplicate Requirement: {}", requirement.id)));
            }
            let fingerprint = schema::host_fingerprint(
                &requirement.id,
                &Protobuf {
                    descriptors: prost::Message::encode_to_vec(&requirement.types),
                    request: requirement.request.clone(),
                    response: requirement.response.clone(),
                    input: None,
                    output: requirement.watch.then(|| requirement.response.clone()),
                },
            )
            .map_err(|error| Error::Invalid(error.to_string()))?;
            declarations.requires.push(Requirement { id: requirement.id.clone(), fingerprint });
            merge_types(&mut types, requirement.types.clone())?;
        }
        let mut streams = HashSet::new();
        for stream in &self.streams {
            if stream.declaration.id.is_empty() || !streams.insert(&stream.declaration.id) {
                return Err(Error::Invalid(format!("Duplicate or Empty Stream: {}", stream.declaration.id)));
            }
            let mut declaration = stream.declaration.clone();
            let observers = self
                .observers
                .iter()
                .filter(|observer| {
                    observer.stream == declaration.id
                        || observer.members.lock().expect("collection members lock is healthy").contains_key(&declaration.id)
                })
                .collect::<Vec<_>>();
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
                if let Some(Err(error)) = &observer.columns {
                    return Err(Error::Invalid(format!("{error} (stream {})", declaration.id)));
                }
                declaration.interval = observer.interval;
                declaration.interval_setting = observer.interval_setting.clone();
                declaration.window_setting = observer.window_setting.clone();
            }
            declarations.streams.push(declaration);
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
            declarations.sections.push(section.clone());
            merge_types(&mut types, definitions.clone())?;
        }
        let mut validation = types.clone();
        let mut messages: Vec<_> = types.file.iter().flat_map(|file| &file.message_type).collect();
        let mut message_options = false;
        while let Some(message) = messages.pop() {
            message_options |= message.options.is_some();
            messages.extend(&message.nested_type);
        }
        if message_options && !validation.file.iter().any(|file| file.name.as_deref() == Some("google/protobuf/descriptor.proto")) {
            merge_types(&mut validation, schema::descriptor_options())?;
        }
        let pool = DescriptorPool::from_file_descriptor_set(validation)
            .map_err(|error| Error::Invalid(format!("Invalid Protobuf Definitions: {error}")))?;
        let names = declarations
            .operations
            .iter()
            .flat_map(|operation| [&operation.request, &operation.response])
            .chain(declarations.streams.iter().map(|stream| &stream.row))
            .chain(declarations.sections.iter().map(|section| &section.r#type))
            .chain(self.messages.iter().map(|(name, _)| name));
        for name in names {
            if pool.get_message_by_name(name).is_none() {
                return Err(Error::Invalid(format!("Missing Protobuf Message: {name}")));
            }
        }
        declarations.types = Some(types);
        schema::plugin_lanes(&self.id, &declarations).map_err(|error| Error::Invalid(error.to_string()))?;
        schema::plugin_collections(&declarations).map_err(|error| Error::Invalid(error.to_string()))?;
        declarations.digest = schema::declarations_digest(&declarations);
        Ok(declarations)
    }

    pub(crate) fn requirements(declarations: &Declarations) -> Arc<HashMap<String, Vec<u8>>> {
        Arc::new(declarations.requires.iter().map(|requirement| (requirement.id.clone(), requirement.fingerprint.clone())).collect())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, SettingDeclaration, SettingValue, SettingsDeclarations, setting_value};
    use prost_types::Duration;

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
        panic!("Declarations Inspection Entered A Handler");
    }

    #[test]
    fn declarations_describes_values_without_running_handlers_or_cleanup() {
        let plugin = Plugin::new("declarations", "1.0.0")
            .register(|ctx| {
                ctx.register(Operation::new("declarations.read", not_called).visibility(Visibility::Protected))?;
                ctx.register(crate::plugin::Contribution(Settings { interval_ms: 250 }))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
            .on_stop(|| panic!("Declarations Inspection Entered Cleanup"));
        let declarations = plugin.declarations().unwrap();
        assert_eq!(declarations.operations.len(), 1);
        assert_eq!(declarations.operations[0].id, "declarations.read");
        assert_eq!(declarations.sections.len(), 1);
        let settings = <Settings as prost::Message>::decode(declarations.sections[0].payload.as_slice()).unwrap();
        assert_eq!(settings.interval_ms, 250);
    }

    #[test]
    fn typed_settings_preserve_presence() {
        let declarations = SettingsDeclarations {
            values: vec![
                SettingDeclaration {
                    name: "flag".into(),
                    default: Some(SettingValue {
                        value: Some(setting_value::Value::Flag(false)),
                    }),
                    ..Default::default()
                },
                SettingDeclaration {
                    name: "text".into(),
                    default: Some(SettingValue {
                        value: Some(setting_value::Value::Text(String::new())),
                    }),
                    ..Default::default()
                },
                SettingDeclaration {
                    name: "limit".into(),
                    default: Some(SettingValue {
                        value: Some(setting_value::Value::Integer(7)),
                    }),
                    minimum: Some(SettingValue {
                        value: Some(setting_value::Value::Integer(0)),
                    }),
                    maximum: Some(SettingValue {
                        value: Some(setting_value::Value::Integer(10)),
                    }),
                    ..Default::default()
                },
            ],
        };
        let reported = Plugin::new("settings", "1.0.0")
            .register(|ctx| {
                for declaration in declarations.clone().values {
                    ctx.register(SettingsDeclarations { values: vec![declaration] })?;
                }
                Ok(())
            })
            .expect("Register Plugin Declarations")
            .declarations()
            .unwrap();
        let decoded = <SettingsDeclarations as prost::Message>::decode(reported.sections[0].payload.as_slice()).unwrap();
        assert_eq!(decoded, declarations);
    }

    #[test]
    fn lanes_declare_capacity_and_refuse_invalid_assignments() {
        let instrument = Lane::serial("instrument");
        let parallel = Lane::concurrent("parallel", 2);
        let open = Lane::unbounded("open");
        let plugin = Plugin::new("example", "1.0.0")
            .lane(instrument.clone())
            .lane(parallel)
            .lane(open)
            .register(|ctx| ctx.register(Operation::new("example.measure", empty).visibility(Visibility::Public).lane(&instrument)))
            .unwrap();
        let declarations = plugin.declarations().unwrap();
        let encoded = declarations.lanes.iter().map(prost::Message::encode_to_vec).collect::<Vec<_>>();
        assert_eq!(
            encoded,
            vec![b"\x0a\x0ainstrument\x10\x01".to_vec(), b"\x0a\x08parallel\x18\x02".to_vec(), b"\x0a\x04open\x20\x01".to_vec()]
        );
        assert_eq!(declarations.operations[0].lane, "instrument");
        assert!(Plugin::new("example", "1.0.0").lane(instrument.clone()).lane(instrument.clone()).declarations().is_err());
        for lane in [Lane::serial(""), Lane::serial("Open"), Lane::serial("harmos.resident"), Lane::concurrent("instrument", 0)] {
            assert!(Plugin::new("example", "1.0.0").lane(lane).declarations().is_err());
        }
        assert!(
            Plugin::new("example", "1.0.0")
                .register(|ctx| ctx.register(Operation::new("example.measure", empty).visibility(Visibility::Public).lane(&instrument)))
                .unwrap()
                .declarations()
                .is_err()
        );
    }

    #[test]
    fn duplicate_operations_are_rejected_at_declarations_construction() {
        let plugin = Plugin::new("test", "1.0.0")
            .register(|ctx| {
                ctx.register(Operation::new("test.duplicate", empty).visibility(Visibility::Protected))?;
                ctx.register(Operation::new("test.duplicate", empty).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations");
        assert!(plugin.declarations().is_err());
    }

    #[test]
    fn observer_must_own_one_declared_stream() {
        let stream = Stream::<()>::new("test.samples");
        let observer = Observer::new(&stream, observe).every(Duration { seconds: 1, nanos: 0 });
        assert!(
            Plugin::new("test", "1.0.0")
                .register(|ctx| {
                    ctx.register(observer)?;
                    Ok(())
                })
                .expect("Register Plugin Declarations")
                .declarations()
                .is_err()
        );
        let first = Observer::new(&stream, observe).every(Duration { seconds: 1, nanos: 0 });
        let second = Observer::new(&stream, observe).every(Duration { seconds: 1, nanos: 0 });
        assert!(
            Plugin::new("test", "1.0.0")
                .register(|ctx| {
                    ctx.register(stream)?;
                    ctx.register(first)?;
                    ctx.register(second)?;
                    Ok(())
                })
                .expect("Register Plugin Declarations")
                .declarations()
                .is_err()
        );
    }
}

#[cfg(test)]
mod registration_tests {
    use super::{Contribution, Plugin, Registrar, Registration};
    use crate::plugin::Operation;
    use crate::{Context, Message, Result, Visibility};

    mod settings {
        use super::{Contribution, Registrar, Registration};

        pub fn catalog() -> impl Registration {
            |ctx: &mut Registrar<'_>| ctx.register(Contribution(()))
        }
    }

    #[derive(crate::Message)]
    struct Echo {
        value: String,
    }

    async fn echo(_: &Context, request: Echo) -> Result<Echo> {
        Ok(request)
    }

    #[test]
    fn module_catalogs_accumulate_typed_values_without_entering_handlers() {
        let plugin = Plugin::new("registration", "1.0.0")
            .register(|ctx| {
                ctx.register(settings::catalog())?;
                ctx.register(Operation::new("registration.echo", echo).visibility(Visibility::Public))?;
                Ok(())
            })
            .unwrap();
        let declarations = plugin.declarations().unwrap();
        assert_eq!(declarations.sections.len(), 1);
        assert_eq!(declarations.operations.len(), 1);
        assert_eq!(declarations.operations[0].request, Echo::name());
    }
}
