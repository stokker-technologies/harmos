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
use harmos::{Context, Error, Message, Observer, Operation, Plugin, Result, Stream, Visibility, clock};
use harmos_protocol::{
    plugin::host::{InvocationsWatchRequest, InvocationsWatchResponse, SettingsListRequest},
    v1,
};
use prost::Message as _;
use prost_types::{Duration as ProtoDuration, Timestamp};
use std::{env, fs, future, num::NonZeroUsize, path::PathBuf, sync::Mutex, time::Duration};
use tokio::sync::Notify;

/* === Constants === */

static HELD: Mutex<Vec<(Context, harmos::Watching<InvocationsWatchResponse>)>> = Mutex::new(Vec::new());

static READING_RELEASED: Notify = Notify::const_new();

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Message)]
struct Input {}

#[derive(Message)]
struct Evidence {
    resource: u64,
    at: Option<Timestamp>,
}

#[derive(Message)]
struct Isolation {
    absent: bool,
}

struct Reading {
    cleanup: PathBuf,
}

#[derive(Message)]
pub struct SampleReading {
    pub sequence: u64,
    pub seconds: i64,
    pub setting: i64,
    pub failure: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Drop for Reading {
    fn drop(&mut self) {
        fs::write(&self.cleanup, b"released").expect("Write Reading Cleanup Evidence");
        READING_RELEASED.notify_one();
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() -> Result<()> {
    let cleanup = env::args().nth(1).expect("Cleanup Evidence Path");
    let reading_cleanup = PathBuf::from(&cleanup).with_extension("reading");
    let reading = Stream::<Isolation>::new("execution.reading");
    let observer = Observer::new(&reading, async move |ctx: &Context| {
        ctx.set(Reading { cleanup: reading_cleanup.clone() });
        println!("observer_reading");
        future::pending::<Result<Isolation>>().await
    })
    .every(ProtoDuration { seconds: 1, nanos: 0 });
    let children = Stream::<SampleReading>::new("execution.children");
    let child_observer = Observer::new(&children, samples_children).every(ProtoDuration { seconds: 1, nanos: 0 });
    let device = Stream::<Isolation>::new("execution.device").publish_on(async |context: &Context, _: harmos::Publisher<Isolation>| {
        context.cancelled().await;
        Ok(())
    });
    Plugin::new("execution", "1.0.0")
        .stream(children)
        .observer(child_observer)
        .stream(device)
        .requires::<SettingsListRequest>()
        .requires::<InvocationsWatchRequest>()
        .operation(Operation::new("execution.stale", stale_context).visibility(Visibility::Protected))
        .operation(Operation::new("execution.clock", clock_and_resource).visibility(Visibility::Protected))
        .operation(Operation::new("execution.isolation", isolation).visibility(Visibility::Protected))
        .operation(Operation::new("execution.reading_released", reading_released).visibility(Visibility::Protected))
        .stream(reading)
        .observer(observer)
        .requires::<clock::TimeSleepRequest>()
        .requires::<clock::TimeGetRequest>()
        .on_stop(move || {
            fs::write(&cleanup, b"cleaned")?;
            Ok(())
        })
        .remote()
        .serve()
}

async fn clock_and_resource(ctx: &Context, _: Input) -> Result<Evidence> {
    ctx.set(42_u64);
    ctx.sleep(Duration::from_secs(3600)).await?;
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

async fn samples_children(context: &Context) -> Result<SampleReading> {
    let (time, settings) = tokio::join!(context.now(), context.settings());
    let (seconds, failure) = match time {
        Ok(time) => (time.seconds, Vec::new()),
        Err(Error::Failure(failure)) => (-1, failure.encode_to_vec()),
        Err(error) => return Err(error),
    };
    let setting = match settings?.value.and_then(|values| values.values.get("threshold").cloned()).and_then(|value| value.value) {
        Some(v1::setting_value::Value::Integer(value)) => value,
        _ => return Err(Error::Invalid("Read Samples: Integer Setting Required".into())),
    };
    let mut watching = context.watch(InvocationsWatchRequest { instance: 1 }, NonZeroUsize::new(4096).unwrap()).await?;
    if watching.next().await?.is_none() {
        return Err(Error::Invalid("Read Samples: Watch Ended Before Item".into()));
    }
    HELD.lock().unwrap().push((context.clone(), watching));
    Ok(SampleReading {
        sequence: context.sequence(),
        seconds,
        setting,
        failure,
    })
}

async fn stale_context(_: &Context, _: Input) -> Result<Isolation> {
    let (saved, mut watch) =
        HELD.lock().unwrap().pop().ok_or_else(|| Error::Invalid("Check Samples Context: No Retained Context".into()))?;
    let refused = matches!(saved.now().await, Err(Error::Transport(status)) if status.code() == tonic::Code::InvalidArgument);
    let watch_refused = watch.next().await.is_err();
    let settings_refused = saved.settings().await.is_err();
    let scope_preserved = saved.scope()? == "scope";
    Ok(Isolation {
        absent: refused && watch_refused && settings_refused && scope_preserved,
    })
}

/* === Tests === */
