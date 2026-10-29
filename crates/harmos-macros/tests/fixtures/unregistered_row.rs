/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


use harmos::{Application, state::Never, streams::StreamManager};


struct State;

struct Studio;

#[derive(Clone)]
struct OutsideTheCatalog;


#[harmos::lanes]
enum Lanes {
    #[default]
    Open,
}


impl Application for Studio {
    type Lanes = Lanes;
    type State = State;
    type Transaction = Never;
    type Record = ();
    type Streams = ();
}


fn publish(streams: &StreamManager<Studio>) {
    streams.publish(OutsideTheCatalog);
}

fn main() {}
