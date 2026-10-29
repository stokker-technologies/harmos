/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use harmos_runtime::{Application, state::Never};

pub struct App;

impl harmos_runtime::Lifecycle for App {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl Application for App {
    type Lanes = super::Lanes;
    type State = ();
    type Transaction = Never;
    type Record = ();
    type Streams = ();
}
