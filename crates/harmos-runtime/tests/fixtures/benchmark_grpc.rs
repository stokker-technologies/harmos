/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Serve Benchmark Commands In A Separate Process Until The Parent Closes Stdin.

/* === Modules === */

#[path = "benchmark_protocol.rs"]
mod protocol;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io::{Read, Write};
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    let executor = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    executor.block_on(async {
        let preparation_bytes = NonZeroUsize::new(64 * 1024 * 1024).unwrap();
        let settings = harmos_runtime::operations::Settings {
            queued: NonZeroUsize::new(64).unwrap(),
            lifecycle_queued: NonZeroUsize::new(16).unwrap(),
            preparation_bytes: Some(preparation_bytes),
        };
        let runtime = harmos_runtime::Runtime::<protocol::Model>::builder((), ())
            .register(protocol::Lanes::catalog())
            .operation_settings(settings)
            .runtime()
            .await
            .unwrap();
        runtime.operations.register_wire_operation::<protocol::Echoing>("benchmark", protocol::Lanes::Open, Arc::new(())).unwrap();
        let attached = runtime.grpc().attach(protocol::settings()).await.unwrap();
        println!("{}", attached.local_addr());
        std::io::stdout().flush().unwrap();

        tokio::task::spawn_blocking(|| {
            let mut bytes = [0_u8; 1];
            while std::io::stdin().read(&mut bytes).unwrap() != 0 {}
        })
        .await
        .unwrap();
        drop(attached);
        runtime.stop().await.unwrap();
    });
}

/* === Tests === */
