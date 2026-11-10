/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// SDK Execution Test Plugin

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos::{Context, Message, Operation, Plugin, Result, clock};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Message)]
struct Input {}

#[derive(Message)]
struct Evidence {
    resource: u64,
    at: Option<prost_types::Timestamp>,
}

#[derive(Message)]
struct Isolation {
    absent: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() -> Result<()> {
    let cleanup = std::env::args().nth(1).expect("Cleanup Evidence Path");
    Plugin::new("execution", "1.0.0")
        .operation(Operation::new("execution.clock", clock_and_resource))
        .operation(Operation::new("execution.isolation", isolation))
        .requires::<clock::Sleep>()
        .requires::<clock::Now>()
        .on_stop(move || {
            std::fs::write(&cleanup, b"cleaned")?;
            Ok(())
        })
        .remote()
        .serve()
}

async fn clock_and_resource(ctx: &Context, _: Input) -> Result<Evidence> {
    ctx.set(42_u64);
    ctx.sleep(std::time::Duration::from_secs(3600)).await?;
    let at = ctx.now().await?;
    Ok(Evidence {
        resource: *ctx.get::<u64>()?,
        at: Some(at),
    })
}

async fn isolation(ctx: &Context, _: Input) -> Result<Isolation> {
    Ok(Isolation { absent: ctx.get::<u64>().is_err() })
}

/* === Tests === */
