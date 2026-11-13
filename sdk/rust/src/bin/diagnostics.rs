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
use harmos::{Context, Message, Observer, Operation, Plugin, Result, Stream, clock};

/* === Constants === */

static READING_RELEASED: tokio::sync::Notify = tokio::sync::Notify::const_new();

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

struct Reading {
    cleanup: std::path::PathBuf,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Drop for Reading {
    fn drop(&mut self) {
        std::fs::write(&self.cleanup, b"released").expect("Write Reading Cleanup Evidence");
        READING_RELEASED.notify_one();
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() -> Result<()> {
    let cleanup = std::env::args().nth(1).expect("Cleanup Evidence Path");
    let reading_cleanup = std::path::PathBuf::from(&cleanup).with_extension("reading");
    let reading = Stream::<Isolation>::new("execution.reading");
    let observer = Observer::new(&reading, async move |ctx: &Context| {
        ctx.set(Reading { cleanup: reading_cleanup.clone() });
        println!("observer_reading");
        std::future::pending::<Result<Isolation>>().await
    })
    .every(prost_types::Duration { seconds: 1, nanos: 0 });
    Plugin::new("execution", "1.0.0")
        .operation(Operation::new("execution.clock", clock_and_resource).visibility(harmos_protocol::v1::Visibility::Protected))
        .operation(Operation::new("execution.isolation", isolation).visibility(harmos_protocol::v1::Visibility::Protected))
        .operation(Operation::new("execution.reading_released", reading_released).visibility(harmos_protocol::v1::Visibility::Protected))
        .stream(reading)
        .observer(observer)
        .requires::<clock::TimeSleepRequest>()
        .requires::<clock::TimeGetRequest>()
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

async fn reading_released(_: &Context, _: Input) -> Result<Isolation> {
    READING_RELEASED.notified().await;
    Ok(Isolation { absent: true })
}

async fn isolation(ctx: &Context, _: Input) -> Result<Isolation> {
    Ok(Isolation { absent: ctx.get::<u64>().is_err() })
}

/* === Tests === */
