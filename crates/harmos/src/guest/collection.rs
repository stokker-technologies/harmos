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
use std::collections::BTreeSet;
// Third Party Imports

// Local Imports
use super::{Error, Host, Registration};
use crate::{Application, Never, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Collection;

struct Cleanup {
    runtime: Option<Runtime<Collection>>,
    executor: tokio::runtime::Handle,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Application for Collection {
    type State = ();
    type Transaction = Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            self.executor.spawn(async move {
                if let Err(error) = runtime.stop().await {
                    tracing::error!(%error, "catalog runtime shutdown failed after cancellation");
                }
            });
        }
    }
}

/* === Public Functions === */
pub async fn registrations(bytes: &[u8]) -> Result<Vec<Registration>, Error> {
    let runtime = Runtime::<Collection>::builder((), ())
        .runtime()
        .await
        .map_err(|error| Error::Guest {
            message: format!("catalog runtime could not start: {error}"),
        })?;
    let mut cleanup = Cleanup {
        runtime: Some(runtime),
        executor: tokio::runtime::Handle::current(),
    };
    let result = async {
        let host = Host::new(
            cleanup
                .runtime
                .as_ref()
                .expect("runtime is owned until cleanup"),
        )?;
        let guest = host
            .load(bytes)?
            .register_with_limits(Some(super::host::store_limits()))
            .await?;
        let entries = guest.registrations();
        let mut namespaces = BTreeSet::new();
        let mut size = 0usize;
        if entries.len() > 256 {
            return Err(invalid("catalog exceeds 256 namespaces"));
        }
        for entry in entries {
            if !harmos_contract::artifact_subset(&entry.namespace)
                || entry.schema == 0
                || !namespaces.insert(&entry.namespace)
            {
                return Err(invalid(
                    "catalog has an invalid or duplicate namespace/schema",
                ));
            }
            size = size
                .checked_add(entry.namespace.len())
                .and_then(|size| size.checked_add(entry.payload.len()))
                .filter(|size| *size <= 16 * 1024 * 1024)
                .ok_or_else(|| invalid("catalog exceeds 16 MiB"))?;
        }
        let mut entries = entries.to_vec();
        entries.sort_by(|left, right| left.namespace.cmp(&right.namespace));
        Ok(entries)
    }
    .await;
    let runtime = cleanup
        .runtime
        .take()
        .expect("runtime has not been stopped");
    // The task retains shutdown ownership even if this final await is cancelled,
    // so the runtime is moved into it rather than borrowed across the spawn.
    cleanup
        .executor
        .spawn(async move { runtime.stop().await })
        .await
        .map_err(|error| invalid(&format!("catalog runtime shutdown task failed: {error}")))?
        .map_err(|error| invalid(&format!("catalog runtime shutdown failed: {error}")))?;
    result
}

/* === Private Functions === */
fn invalid(message: &str) -> Error {
    Error::Guest {
        message: message.to_owned(),
    }
}

/* === Tests === */
