/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Light Constructors for Generated Storage Messages.

/* === Implementations === */

impl From<crate::storage::directory_event::Event> for crate::storage::DirectoryEvent {
    fn from(event: crate::storage::directory_event::Event) -> Self {
        Self { event: Some(event) }
    }
}
