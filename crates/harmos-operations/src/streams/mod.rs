/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod publish;

/* === Re-Exports === */

pub use publish::{FollowingRow, Publish};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[harmos::catalog(harmos_path = crate)]
pub enum Streams {
    Publish(Publish),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Publishes one row on a declared stream through the host's `host.publish` operation.
#[cfg(feature = "lifecycle")]
pub fn publish_on<H, S>(host: &H, row: &S::Row) -> impl std::future::Future<Output = std::result::Result<(), H::Error>> + Send
where
    H: harmos_sdk::operations::Host,
    S: harmos_domain::StreamSpec,
    S::Row: harmos_domain::Message,
{
    use harmos_domain::Message;
    let published = host.call::<Publish>(Publish {
        stream: S::ID.to_owned(),
        schema: S::VERSION,
        sequence: None,
        published_unix_micros: None,
        context: None,
        payload: row.encode(),
        following: Vec::new(),
    });
    async move {
        published.await?;
        Ok(())
    }
}

/* === Private Functions === */

/* === Tests === */
