/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retain Native System Story Traces Through Settlement And Unwinding

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

// Third Party Imports
use tracing_perfetto_file::{FlushGuard, PerfettoLayer};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[must_use = "keep the report alive until the story's work settles"]
pub struct Report {
    pub(crate) guard: Option<Arc<FlushGuard>>,
    pub(crate) story: Option<tracing::Span>,
    pub(crate) dispatch: Option<tracing::Dispatch>,
}

struct Collector {
    guard: Arc<FlushGuard>,
    dispatch: tracing::Dispatch,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Drop for Report {
    fn drop(&mut self) {
        let story = self.story.take();
        let Some(guard) = self.guard.take() else {
            return;
        };
        let close = || {
            drop(story);
            guard.flush()
        };
        let flushed = match self.dispatch.as_ref() {
            Some(dispatch) => tracing::dispatcher::with_default(dispatch, close),
            None => close(),
        };
        let Err(error) = flushed else {
            return;
        };
        if std::thread::panicking() {
            eprintln!("system trace output failed during unwinding: {error}");
            return;
        }
        panic!("the native system trace could not be retained: {error}");
    }
}

/* === Public Functions === */

pub fn system() -> Report {
    let directory = std::env::var_os("STKR_TEST_REPORT_DIR").map(PathBuf::from);
    let traces = directory.map(|directory| directory.join("traces"));
    capture(traces.as_deref())
}

pub fn capture(directory: Option<&Path>) -> Report {
    static EXPORTER: OnceLock<Collector> = OnceLock::new();
    let Some(directory) = directory else {
        return Report { guard: None, story: None, dispatch: None };
    };
    let initialize = || initialize(directory);
    let collector = EXPORTER.get_or_init(initialize);

    let thread = std::thread::current();
    let name = thread.name().unwrap_or("system");
    let process = std::process::id();
    let story = tracing::dispatcher::with_default(&collector.dispatch, || tracing::debug_span!("system.story", story = name, process));
    Report {
        guard: Some(Arc::clone(&collector.guard)),
        story: Some(story),
        dispatch: Some(collector.dispatch.clone()),
    }
}

pub(crate) fn instrumented(metadata: &tracing::Metadata<'_>) -> bool {
    if !metadata.is_span() {
        return matches!(metadata.target(), "harmos::operations" | "harmos::state" | "harmos::carrier" | "harmos::grpc");
    }
    let prefixes = ["operations.", "state.", "storage.", "streams.", "carrier.", "grpc.", "sidecar.", "system.story"];
    prefixes.iter().any(|prefix| metadata.name().starts_with(prefix))
}

/* === Private Functions === */

fn initialize(directory: &Path) -> Collector {
    std::fs::create_dir_all(directory).expect("the system trace directory is writable");
    let process = std::process::id();
    let name = std::thread::current().name().unwrap_or("system").to_owned();
    let letters = name.chars();
    let characters = letters.map(|character| {
        let letter = character.is_ascii_alphanumeric();
        if letter { character } else { '_' }
    });
    let story: String = characters.collect();
    let prefix = format!("{story}-{process}-");
    let temporary = tempfile::Builder::new()
        .prefix(&prefix)
        .suffix(".pftrace")
        .tempfile_in(directory)
        .expect("each system process owns a distinct native trace");
    let (file, _) = temporary.keep().expect("the system trace persists after the story completes");

    let (layer, guard) =
        PerfettoLayer::builder(file).with_debug_annotations().with_source_locations().with_poll_slices().with_counters().build();
    let layer = layer.with_filter(filter_fn(instrumented));
    let subscriber = tracing_subscriber::registry().with(layer);
    let dispatch = tracing::Dispatch::new(subscriber);
    tracing::dispatcher::set_global_default(dispatch.clone()).expect("the system process installs one trace subscriber");
    Collector { guard: Arc::new(guard), dispatch }
}

/* === Tests === */
