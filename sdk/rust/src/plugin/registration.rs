/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Declaration Registration

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::marker::PhantomData;

// Third Party Imports

// Local Imports
use super::{Plugin, Required};
use crate::{
    Error, Message, OperationRequest, Result, message,
    operation::Operation,
    stream::{Collection, Observer, Stream},
};
use harmos_protocol::model::{Section, SettingsDeclarations};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Declaration-Only Admission Context; No Runtime Facilities Are Available.
pub struct Registrar<'a> {
    pub(super) plugin: &'a mut Plugin,
}

/// An Opaque Product Declaration Section.
pub struct Contribution<T>(pub T);

/// A Reachable Payload Descriptor Without an Executable Declaration.
pub struct Descriptor<T>(PhantomData<fn() -> T>);

/// A Typed Dependency on Another Admitted Operation.
pub struct Dependency<T>(PhantomData<fn() -> T>);

/* === Enums === */

/* === Traits === */

pub trait Registration {
    fn register(self, ctx: &mut Registrar<'_>) -> Result<()>;
}

/* === Implementations === */

impl Registrar<'_> {
    pub fn register<T: Registration>(&mut self, item: T) -> Result<()> {
        item.register(self)
    }
}

impl<T> Descriptor<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Dependency<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Default for Descriptor<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Default for Dependency<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl Registration for Operation {
    fn register(self, ctx: &mut Registrar<'_>) -> Result<()> {
        ctx.plugin.operations.push(self);
        Ok(())
    }
}

impl<T: Message> Registration for Stream<T> {
    fn register(self, ctx: &mut Registrar<'_>) -> Result<()> {
        ctx.plugin.streams.push(self.definition);
        Ok(())
    }
}

impl Registration for Observer {
    fn register(self, ctx: &mut Registrar<'_>) -> Result<()> {
        ctx.plugin.observers.push(self);
        Ok(())
    }
}

impl Registration for Collection {
    fn register(mut self, ctx: &mut Registrar<'_>) -> Result<()> {
        let members = self.observer.members.lock().expect("collection members lock is healthy");
        let id = members.keys().next().ok_or_else(|| Error::Invalid("Register Collection: declare at least one stream".into()))?.clone();
        drop(members);
        self.observer.stream = id.clone();
        for mut stream in self.streams {
            if stream.producer.is_some() {
                return Err(Error::Invalid(format!(
                    "Register Collection `{id}`: remove member `{}`'s device producer",
                    stream.declaration.id
                )));
            }
            stream.declaration.collection = id.clone();
            ctx.plugin.streams.push(stream);
        }
        ctx.plugin.observers.push(self.observer);
        Ok(())
    }
}

impl<T: Message> Registration for Contribution<T> {
    fn register(self, ctx: &mut Registrar<'_>) -> Result<()> {
        ctx.plugin.sections.push((
            Section {
                r#type: T::name(),
                payload: self.0.encode_to_vec(),
            },
            T::types(),
        ));
        Ok(())
    }
}

impl Registration for SettingsDeclarations {
    fn register(self, ctx: &mut Registrar<'_>) -> Result<()> {
        if let Some((section, _)) = ctx.plugin.sections.iter_mut().find(|(section, _)| section.r#type == SettingsDeclarations::name()) {
            let mut declarations = <SettingsDeclarations as prost::Message>::decode(section.payload.as_slice())
                .map_err(|error| Error::Invalid(format!("Register Settings: {error}")))?;
            declarations.values.extend(self.values);
            section.payload = prost::Message::encode_to_vec(&declarations);
            return Ok(());
        }
        ctx.register(Contribution(self))
    }
}

impl<T: Message> Registration for Descriptor<T> {
    fn register(self, ctx: &mut Registrar<'_>) -> Result<()> {
        ctx.plugin.messages.push((T::name(), T::types()));
        Ok(())
    }
}

impl<T: OperationRequest> Registration for Dependency<T> {
    fn register(self, ctx: &mut Registrar<'_>) -> Result<()> {
        let mut types = T::types();
        message::merge_descriptor_types(&mut types, T::Response::types());
        ctx.plugin.requirements.push(Required {
            id: T::ID.into(),
            request: T::name(),
            response: T::Response::name(),
            watch: T::WATCH,
            types,
        });
        Ok(())
    }
}

impl<F> Registration for F
where
    F: FnOnce(&mut Registrar<'_>) -> Result<()>,
{
    fn register(self, ctx: &mut Registrar<'_>) -> Result<()> {
        self(ctx)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
