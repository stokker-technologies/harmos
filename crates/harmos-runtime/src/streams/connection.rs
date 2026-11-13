/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Connection-Owned Stream Registration

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::{Arc, Mutex};
// Third Party Imports
// Local Imports
use super::definition::{Contract, Wired};
use super::publications::{publication_definitions, validate_existing};
use super::{PublishedStream, StreamManager, StreamRegistrationError, StreamWriter};
use crate::{Application, state::Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct ConnectionStreams {
    leases: Vec<Arc<StreamWriter>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<A: Application> StreamManager<A> {
    pub(crate) fn connect_publications(
        &self, owner: &str, declarations: impl IntoIterator<Item = harmos_operations::StreamDeclaration>,
    ) -> Result<ConnectionStreams, StreamRegistrationError> {
        let definitions = publication_definitions(owner, declarations)?;
        let validations = definitions
            .iter()
            .map(|definition| {
                crate::schema::Validator::from_schema(definition.schema(), &self.schema_settings)
                    .map_err(crate::schema::Error::from)
                    .map(Arc::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(StreamRegistrationError::Closed);
        }
        let _visibility = self.windows.visibility.write().expect("stream visibility poisoned");
        let mut windows = self.windows.windows.write().expect("stream registry poisoned");
        self.windows.retire(&mut windows);
        validate_existing(&windows, &definitions, true)?;
        let mut leases = Vec::with_capacity(definitions.len());
        for (definition, validation) in definitions.into_iter().zip(validations) {
            let revisions = windows.entry(Arc::<str>::from(definition.id())).or_default();
            let window = revisions.wires.get(&definition.version()).map_or_else(
                || {
                    Arc::new(
                        super::window::Window::wired(
                            definition.window(),
                            Position::ORIGIN,
                            self.windows.sequence.clone(),
                            self.windows.pump.clone(),
                        )
                        .visible_with(self.windows.visibility.clone()),
                    )
                },
                |previous| previous.window.clone(),
            );
            let active = Arc::new(Mutex::new(true));
            let wired = Arc::new(Wired {
                validation: Some(validation),
                definition,
                window,
                publisher: Mutex::new(Arc::downgrade(&active)),
                #[cfg(feature = "plugin-guest")]
                staged: None,
            });
            revisions.definitions.insert(wired.definition.version(), Contract::of(&wired.definition));
            revisions.current = Some(wired.definition.version());
            revisions.wires.insert(wired.definition.version(), wired.clone());
            leases.push(Arc::new(StreamWriter {
                stream: PublishedStream {
                    wired,
                    closed: self.closed.clone(),
                    pool: self.windows.publications.clone(),
                    ingest: self.windows.ingest.clone(),
                },
                active,
                applied: self.applied.clone(),
                sequence: std::sync::atomic::AtomicU64::new(0),
            }));
        }
        Ok(ConnectionStreams { leases })
    }
}
impl ConnectionStreams {
    pub(crate) fn leases(&self) -> &[Arc<StreamWriter>] {
        &self.leases
    }
}
impl Drop for ConnectionStreams {
    fn drop(&mut self) {
        for lease in &self.leases {
            lease.revoke();
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
