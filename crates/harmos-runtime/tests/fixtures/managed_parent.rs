/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host Process Whose Death Must End Its Managed Plugin

/* === Imports === */

// Standard Library Imports
use std::{io::Write, num::NonZeroUsize, path::PathBuf, time::Duration};

// Local Imports
use harmos_runtime::plugin::grpc::{Managed, Placement, PlacementSettings};

/* === Structs === */

struct Model;

/* === Implementations === */

impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl harmos_runtime::Application for Model {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
}

/* === Public Functions === */

/// Attach the Plugin Named by the Arguments, Report Its Process, Then Wait to Be Killed.
fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let executable = PathBuf::from(arguments.next().expect("managed plugin executable"));
    let arguments = arguments.collect();
    let executor = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    executor.block_on(async {
        let runtime = harmos_runtime::Runtime::<Model>::launch_with((), (), |_| {}).await.unwrap();
        let placement = Placement::Managed(Managed {
            executable,
            arguments,
            environment: Default::default(),
        });
        let settings = PlacementSettings {
            connect_timeout: Duration::from_secs(30),
            reconnect_delay: Duration::from_secs(1),
            stop_grace: Duration::from_secs(30),
            stream_window: NonZeroUsize::new(8).unwrap(),
            interval_window: std::time::Duration::from_secs(1),
            interval_lag: Default::default(),
        };
        let attached = runtime.attach_grpc(placement, settings).await.unwrap();
        let process = attached.activation().unwrap().process_id.unwrap();
        let mut stdout = std::io::stdout();
        writeln!(stdout, "plugin pid {process}").unwrap();
        stdout.flush().unwrap();
        std::future::pending::<()>().await;
    });
}
