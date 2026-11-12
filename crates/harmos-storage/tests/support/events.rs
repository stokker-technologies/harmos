/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use std::{future::Future, pin::Pin};

/* === Public Functions === */

pub(crate) async fn until<F: Future + ?Sized, E: Future>(future: Pin<&mut F>, event: E) -> Result<E::Output, F::Output> {
    tokio::select! {
        biased;
        completed = future => Err(completed),
        observed = event => Ok(observed),
    }
}
