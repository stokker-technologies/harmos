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
use std::{future::Future, io, sync::mpsc, thread};
// Third Party Imports
use tokio::{runtime::Builder, sync::oneshot, task::JoinHandle};
use tracing::{Instrument, instrument::WithSubscriber};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn spawn(
    name: String,
    work: impl Future<Output = ()> + Send + 'static,
) -> io::Result<JoinHandle<()>> {
    let work = work
        .instrument(tracing::Span::current())
        .with_current_subscriber();
    let (started, ready) = mpsc::sync_channel(1);
    thread::Builder::new().name(name).spawn(move || {
        let runtime = match Builder::new_current_thread().enable_all().build() {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = started.send(Err(error));
                return;
            }
        };
        let (finished, completion) = oneshot::channel::<()>();
        let task = runtime.spawn(async move {
            // Dropping this guard also wakes the runtime owner after an abort
            // or panic. It must live until the author's future has dropped.
            let _finished = finished;
            work.await;
        });
        if started.send(Ok(task)).is_err() {
            return;
        }

        let _ = runtime.block_on(completion);
    })?;

    ready.recv().map_err(io::Error::other)?
}

/* === Private Functions === */

/* === Tests === */
