/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::marker::PhantomData;
// Third Party Imports

// Local Imports
use super::Context;
use harmos_contract::operations::{Host, host};
use harmos_contract::{Message, StreamSpec, plugin::Declared};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Publisher<P> {
    context: Context,
    _plugin: PhantomData<fn() -> P>,
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("stream is not in the authored inventory")]
    UndeclaredStream,
    #[error(transparent)]
    Call(#[from] harmos_contract::Refusal),
}

/* === Traits === */

/* === Implementations === */
impl<P> Clone for Publisher<P> {
    fn clone(&self) -> Self {
        Self {
            context: self.context.clone(),
            _plugin: PhantomData,
        }
    }
}

impl Context {
    pub fn publisher<P: Declared>(&self) -> Publisher<P> {
        Publisher {
            context: self.clone(),
            _plugin: PhantomData,
        }
    }
}

impl<P: Declared> Publisher<P> {
    pub async fn publish_on<S: StreamSpec>(&self, row: &S::Row) -> Result<(), PublishError>
    where
        S::Row: Message,
    {
        if !P::STREAMS
            .iter()
            .any(|entry| entry.id == S::ID && entry.schema == S::VERSION)
        {
            return Err(PublishError::UndeclaredStream);
        }
        self.context.calling_context()?;
        let request = host::Publication {
            stream: S::ID.to_owned(),
            schema: S::VERSION,
            sequence: None,
            published_unix_micros: None,
            context: None,
            payload: row.encode_to_vec(),
            following: Vec::new(),
        };
        let published = Host::call_as::<host::Publish>(&self.context, request).await?;
        drop(published);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
