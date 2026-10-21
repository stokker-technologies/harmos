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
use harmos_runtime::{Application, state::Never};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct App<R = ()>(PhantomData<R>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<R: Send + Sync + 'static> Application for App<R> {
    type State = ();
    type Transaction = Never;
    type Record = ();
    type Streams = ();
    type Resources = R;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
