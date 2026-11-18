/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Plugin's Startup Deadline Is the plugins.load Operation's Deadline

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{path::PathBuf, time::Duration};

// Third Party Imports

// Local Imports
use super::{
    invoke_support::pending,
    support::{self, App, Manual, Remote},
};
use harmos_runtime::{
    Runtime,
    clock::{Clock, Source},
    operations::OperationPolicy,
    plugin::grpc::{Error, GrpcPluginPolicy, GrpcPluginSettings, LoadOptions, Managed, Placement},
};

/* === Constants === */

/// The Configured Startup Deadline, Shorter Than the Declared Default, So the Stories Prove Configuration Is Read.
const LOAD: Duration = Duration::from_secs(10);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// The Writing End of a Hung Spawn's Executable: the Spawn's Read Blocks Until It Is Dropped.
#[cfg(target_os = "windows")]
struct Hung(#[allow(dead_code)] tokio::net::windows::named_pipe::NamedPipeServer);

/// The Writing End of a Hung Spawn's Executable: the Spawn's Read Blocks Until It Is Dropped.
#[cfg(unix)]
struct Hung {
    _writer: std::fs::File,
    _directory: tempfile::TempDir,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/// A Runtime on `source` Whose Plugin Loads Are Configured With a `LOAD` Deadline.
async fn configured(source: &Manual) -> Runtime<App> {
    let clock = Clock::with_clock(source.clone());
    crate::support::booted(Default::default(), |setup| {
        let load = OperationPolicy {
            timeout: LOAD,
            attempt_timeout: LOAD,
            ..GrpcPluginPolicy::default().load
        };
        setup.settings().clock(clock).grpc_plugin_settings(GrpcPluginSettings { policy: GrpcPluginPolicy { load } });
    })
    .await
}

/// A Plugin Executable Whose Spawn Hangs Reading It, and the Hung Spawn Once It Has Opened the Executable.
///
/// The spawn's read blocks until the returned [`Hung`] is dropped, so the story ends a read it knows has started.
#[cfg(target_os = "windows")]
fn hung_executable() -> (PathBuf, impl Future<Output = Hung>) {
    let name = format!(r"\\.\pipe\harmos-hung-spawn-{}", std::process::id());
    let server = tokio::net::windows::named_pipe::ServerOptions::new().first_pipe_instance(true).create(&name).unwrap();
    let opened = async move {
        server.connect().await.unwrap();
        Hung(server)
    };
    (PathBuf::from(name), opened)
}

/// A Plugin Executable Whose Spawn Hangs Reading It, and the Hung Spawn Once It Has Opened the Executable.
///
/// The spawn's read blocks until the returned [`Hung`] is dropped, so the story ends a read it knows has started.
#[cfg(unix)]
fn hung_executable() -> (PathBuf, impl Future<Output = Hung>) {
    let directory = tempfile::tempdir().unwrap();
    let fifo = directory.path().join("hung-plugin");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success(), "Create the Hung Executable");
    // Opening the Writing End Blocks Until the Spawn Opens the Reading End; a Plain Thread Never Holds Up the Runtime.
    let (writer, opening) = tokio::sync::oneshot::channel();
    let path = fifo.clone();
    std::thread::spawn(move || {
        let _ = writer.send(std::fs::OpenOptions::new().write(true).open(path));
    });
    let opened = async move {
        Hung {
            _writer: opening.await.unwrap().unwrap(),
            _directory: directory,
        }
    };
    (fifo, opened)
}

/* === Tests === */

#[tokio::test]
async fn a_slow_plugin_misses_the_configured_load_deadline_and_a_per_call_override_lets_it_start() {
    // A Remote Plugin That Describes Itself Only When the Story Releases It, on a Controlled Host Clock.
    let source = Manual::new();
    let runtime = configured(&source).await;
    let remote = Remote::start().await;

    // Under the Configured Policy, the Load Ends at Its Deadline and Names the Plugin.
    let (mut entered, _held) = remote.delay_description();
    let deadline = source.read().instant + LOAD;
    let attaching = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings());
    tokio::pin!(attaching);
    tokio::select! {
        biased;
        _ = &mut attaching => panic!("Load Only Once the Plugin Describes Itself"),
        entered = &mut entered => entered.unwrap(),
    }
    tokio::select! {
        biased;
        _ = &mut attaching => panic!("Keep Loading Until the Deadline"),
        () = source.registered_at(deadline) => {},
    }
    source.advance_by(LOAD);
    let refused = attaching.await;
    assert!(
        matches!(
            &refused,
            Err(Error::LoadDeadline { plugin, timeout, configured_by: "GrpcPluginSettings.policy.load" })
                if *plugin == remote.address && *timeout == LOAD
        ),
        "Fail the Load at the Configured Deadline: {:?}",
        refused.err()
    );

    // A Longer Deadline for This One Load Lets the Same Slow Start Finish Past the Configured Deadline.
    let (mut entered, release) = remote.delay_description();
    let overridden = OperationPolicy {
        timeout: LOAD * 4,
        attempt_timeout: LOAD * 4,
        ..GrpcPluginPolicy::default().load
    };
    let options = LoadOptions { policy: Some(overridden) };
    let attaching = runtime.attach_grpc_with(Placement::Remote(remote.address.clone()), support::settings(), options);
    tokio::pin!(attaching);
    tokio::select! {
        biased;
        _ = &mut attaching => panic!("Load Only Once the Plugin Describes Itself"),
        entered = &mut entered => entered.unwrap(),
    }
    source.advance_by(LOAD * 2);
    assert!(pending(attaching.as_mut()).await, "Keep Loading Past the Configured Deadline Under the Override");
    release.send(()).unwrap();
    let attached = attaching.await.unwrap();
    assert!(attached.activation().is_some(), "Start the Plugin Within the Overridden Deadline");
    attached.stop("Story Complete").await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn a_spawn_that_hangs_fails_at_the_load_deadline_and_names_the_plugin() {
    // A Managed Plugin Whose Executable Never Finishes Reading, on a Controlled Host Clock.
    let source = Manual::new();
    let runtime = configured(&source).await;
    let (executable, opened) = hung_executable();
    let placement = Placement::Managed(Managed {
        executable: executable.clone(),
        directory: None,
        arguments: Vec::new(),
        environment: Default::default(),
    });

    // Spawning Is Part of the Load, So the Load's Deadline Ends the Hung Spawn Instead of Waiting Forever.
    let deadline = source.read().instant + LOAD;
    let attaching = runtime.attach_grpc(placement, support::settings());
    tokio::pin!(attaching);
    let hung = tokio::select! {
        biased;
        _ = &mut attaching => panic!("Keep Spawning While the Executable Read Hangs"),
        hung = opened => hung,
    };
    tokio::select! {
        biased;
        _ = &mut attaching => panic!("Keep Spawning Until the Deadline"),
        () = source.registered_at(deadline) => {},
    }
    source.advance_by(LOAD);
    let refused = attaching.await;
    assert!(
        matches!(
            &refused,
            Err(Error::LoadDeadline { plugin, timeout, .. }) if *plugin == executable.display().to_string() && *timeout == LOAD
        ),
        "Fail the Hung Spawn at the Load Deadline, Naming the Plugin: {:?}",
        refused.err()
    );

    // Ending the Blocked Read Lets the Abandoned Spawn Finish Before the Host Shuts Down.
    drop(hung);
    runtime.stop().await.unwrap();
}
