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

// Third Party Imports

// Local Imports
use harmos::{Application, state::Never, streams::StreamManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct State;

struct Studio;

#[derive(Clone)]
struct OutsideTheCatalog;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Application for Studio {
    type State = State;
    type Transaction = Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

/* === Public Functions === */

/* === Private Functions === */
fn publish(streams: &StreamManager<Studio>) {
    streams.publish(OutsideTheCatalog);
}

fn main() {}

/* === Tests === */
