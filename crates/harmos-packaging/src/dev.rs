/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Coalesce Relevant Source Changes Into Serialized, Recoverable Plugin Builds.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

// Third Party Imports
use notify::{Event, EventKind, RecursiveMode, Watcher};

// Local Imports
use super::metadata::{Inputs, Metadata};
use super::{
    Build, BuildCancellation, Error,
    build::{build_resolved, validate_backend},
};
use crate::config::{BuildSettings, DevSettings};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
pub enum DevEvent {
    Started {
        attempt: u64,
    },
    Finished {
        attempt: u64,
        result: Result<Build, Error>,
    },
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn dev(
    settings: &BuildSettings,
    policy: DevSettings,
    cancellation: &BuildCancellation,
    mut report: impl FnMut(DevEvent) -> Result<(), Error>,
) -> Result<(), Error> {
    validate_backend(settings)?;
    if policy.debounce.is_zero() || Instant::now().checked_add(policy.debounce).is_none() {
        return Err(Error::Settings(
            "development debounce must be positive and representable".into(),
        ));
    }
    let inputs = Arc::new(Mutex::new(Inputs::fallback(settings)?));
    let failure = Arc::new(Mutex::new(None));
    let monitored = inputs.clone();
    let fault = failure.clone();
    let (changes, incoming) = mpsc::sync_channel(1);
    let mut watcher =
        notify::recommended_watcher(move |result: notify::Result<Event>| match result {
            Ok(event)
                if matches!(
                    event.kind,
                    EventKind::Create(_)
                        | EventKind::Modify(_)
                        | EventKind::Remove(_)
                        | EventKind::Any
                        | EventKind::Other
                ) =>
            {
                if event.need_rescan()
                    || event.paths.iter().any(|path| {
                        monitored
                            .lock()
                            .expect("watch inputs are healthy")
                            .relevant(path)
                    })
                {
                    let _ = changes.try_send(());
                }
            }
            Err(error) => {
                fault
                    .lock()
                    .expect("watch failure is healthy")
                    .get_or_insert(error);
                let _ = changes.try_send(());
            }
            _ => {}
        })?;
    let mut watched = BTreeSet::new();
    configure(
        &mut watcher,
        &mut watched,
        &inputs.lock().expect("watch inputs are healthy"),
    )?;
    let mut attempt = 0u64;
    loop {
        if cancellation.is_cancelled() {
            return Ok(());
        }
        if let Some(error) = failure.lock().expect("watch failure is healthy").take() {
            return Err(Error::Watch(error));
        }
        attempt = attempt
            .checked_add(1)
            .ok_or_else(|| Error::Settings("development attempt counter exhausted".into()))?;
        report(DevEvent::Started { attempt })?;
        let resolved = Metadata::read(settings, cancellation)
            .and_then(|metadata| metadata.inputs(settings).map(|inputs| (metadata, inputs)));
        let result = match resolved {
            Ok((metadata, resolved)) => {
                configure(&mut watcher, &mut watched, &resolved)?;
                *inputs.lock().expect("watch inputs are healthy") = resolved;
                build_resolved(settings, cancellation, metadata)
            }
            Err(error) => Err(error),
        };
        if cancellation.is_cancelled() {
            return Ok(());
        }
        report(DevEvent::Finished { attempt, result })?;
        let mut changed = false;
        let mut settled = Instant::now();
        loop {
            if cancellation.is_cancelled() {
                return Ok(());
            }
            if let Some(error) = failure.lock().expect("watch failure is healthy").take() {
                return Err(Error::Watch(error));
            }
            if changed && Instant::now() >= settled {
                break;
            }
            let wait = if changed {
                settled
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(20))
            } else {
                Duration::from_millis(20)
            };
            match incoming.recv_timeout(wait) {
                Ok(()) => {
                    changed = true;
                    settled = Instant::now() + policy.debounce;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(Error::Settings(
                        "source watcher stopped unexpectedly".into(),
                    ));
                }
            }
        }
    }
}

/* === Private Functions === */
fn configure(
    watcher: &mut impl Watcher,
    watched: &mut BTreeSet<PathBuf>,
    inputs: &Inputs,
) -> Result<(), Error> {
    // Watch Source Directories Without Recursively Entering Generated Trees.
    let mut selected = BTreeSet::new();
    let mut pending: Vec<_> = inputs.roots.iter().cloned().collect();
    while let Some(directory) = pending.pop() {
        if !inputs.relevant(&directory) || !selected.insert(directory.clone()) {
            continue;
        }
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    for root in &inputs.roots {
        if let Some(parent) = root.parent() {
            selected.insert(parent.to_owned());
        }
    }
    for file in &inputs.files {
        let mut parent = file.parent();
        while let Some(directory) = parent {
            if directory.is_dir() {
                selected.insert(directory.to_owned());
                break;
            }
            parent = directory.parent();
        }
    }
    // Reattach Watches Removed By The Backend When A Directory Was Recreated.
    for directory in &selected {
        watcher.watch(directory, RecursiveMode::NonRecursive)?;
    }
    for directory in watched.difference(&selected) {
        if let Err(error) = watcher.unwatch(directory)
            && !matches!(error.kind, notify::ErrorKind::WatchNotFound)
        {
            return Err(Error::Watch(error));
        }
    }
    *watched = selected;
    Ok(())
}

/* === Tests === */
