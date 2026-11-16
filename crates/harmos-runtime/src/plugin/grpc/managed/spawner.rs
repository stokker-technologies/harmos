/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Process-Lifetime Managed Child Spawner

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    panic::AssertUnwindSafe,
    sync::{Mutex, mpsc},
};

// Third Party Imports
use tokio::{
    process::{Child, Command},
    runtime::Handle,
    sync::oneshot,
};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Job = Box<dyn FnOnce() + Send>;

/* === Structs === */

/// One Thread Forks Every Managed Child and Hands It to an Owner Before Answering.
///
/// Linux delivers `PR_SET_PDEATHSIG` when the forking thread exits, and runtime workers may
/// retire, so the spawner thread lives for the whole process. A request whose caller is gone is
/// dropped unforked; a child forked for a caller that left meanwhile is dropped into its owner,
/// which kills it and reaps it under the runtime stop participant. A panic answers only the
/// request that raised it. Should the thread end anyway, Linux kills the children it forked and
/// their owners report the exit; the next spawn starts a new thread.
pub(super) struct Spawner {
    requests: Mutex<Option<mpsc::Sender<Job>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Spawner {
    pub(super) const fn new() -> Self {
        Self { requests: Mutex::new(None) }
    }

    /// Fork `command` on the spawner thread and pass the child to `own` there.
    pub(super) async fn spawn<T: Send + 'static>(
        &self, mut command: Command, own: impl FnOnce(Child) -> T + Send + 'static,
    ) -> std::io::Result<T> {
        let runtime = Handle::current();
        let (reply, spawned) = oneshot::channel();
        self.submit(Box::new(move || {
            if reply.is_closed() {
                return;
            }
            let _entered = runtime.enter();
            let _ = reply.send(command.spawn().map(own));
        }))?;
        spawned.await.map_err(|_| std::io::Error::other("the spawner failed while forking; start the plugin again"))?
    }

    fn submit(&self, job: Job) -> std::io::Result<()> {
        let mut requests = self.requests.lock().expect("managed spawner lock is healthy");
        let job = match requests.as_ref() {
            Some(sender) => match sender.send(job) {
                Ok(()) => return Ok(()),
                Err(mpsc::SendError(job)) => job,
            },
            None => job,
        };
        let (sender, received) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("harmos-managed-spawner".into())
            .spawn(move || {
                for job in received {
                    let _ = std::panic::catch_unwind(AssertUnwindSafe(job));
                }
            })
            .map_err(|error| std::io::Error::other(format!("Start Managed Plugin: the process spawner thread could not start ({error}); free system resources and attach again")))?;
        sender.send(job).map_err(|_| std::io::Error::other("the new spawner thread stopped; start the plugin again"))?;
        *requests = Some(sender);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    fn trivial() -> Command {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/C", "exit 0"]);
            command
        } else {
            Command::new("true")
        };
        command.kill_on_drop(true);
        command
    }

    /// Block the Spawner Thread Until the Returned Sender Releases It.
    async fn occupy(spawner: &Spawner) -> std::sync::mpsc::Sender<()> {
        let (release, blocked) = std::sync::mpsc::channel::<()>();
        let (entered, entering) = oneshot::channel();
        spawner
            .submit(Box::new(move || {
                entered.send(()).unwrap();
                let _ = blocked.recv();
            }))
            .unwrap();
        entering.await.unwrap();
        release
    }

    /// Await Every Request Queued Before This One.
    async fn flush(spawner: &Spawner) {
        let (done, flushed) = oneshot::channel();
        spawner.submit(Box::new(move || done.send(()).unwrap())).unwrap();
        flushed.await.unwrap();
    }

    #[tokio::test]
    async fn a_request_whose_caller_left_is_never_forked() {
        // Queue a Spawn Behind a Busy Thread, Then Drop Its Caller.
        let spawner = Spawner::new();
        let release = occupy(&spawner).await;
        let forked = Arc::new(AtomicBool::new(false));
        let observed = forked.clone();
        let spawn = spawner.spawn(trivial(), move |child| {
            observed.store(true, Ordering::SeqCst);
            child
        });
        let mut spawn = Box::pin(spawn);
        assert!(futures_util::poll!(spawn.as_mut()).is_pending(), "Queue the Request Behind the Busy Thread");
        drop(spawn);

        // The Thread Skips the Abandoned Request.
        release.send(()).unwrap();
        flush(&spawner).await;
        assert!(!forked.load(Ordering::SeqCst), "Never Fork for a Caller That Left");
    }

    #[tokio::test]
    async fn a_child_forked_after_its_caller_left_is_dropped_into_its_owner() {
        // The Caller Leaves While the Thread Forks for It.
        static SPAWNER: Spawner = Spawner::new();
        let spawner = &SPAWNER;
        let (left, leaving) = std::sync::mpsc::channel::<()>();
        let (forking, forked) = oneshot::channel();
        let (owned, dropped) = oneshot::channel::<()>();
        struct Owner(#[allow(dead_code)] Child, Option<oneshot::Sender<()>>);
        impl Drop for Owner {
            fn drop(&mut self) {
                let _ = self.1.take().unwrap().send(());
            }
        }
        let spawn = spawner.spawn(trivial(), move |child| {
            forking.send(()).unwrap();
            leaving.recv().unwrap();
            Owner(child, Some(owned))
        });
        let caller = tokio::spawn(spawn);
        forked.await.unwrap();
        caller.abort();
        assert!(caller.await.is_err_and(|error| error.is_cancelled()), "Cancel the Caller While It Awaits the Fork");
        left.send(()).unwrap();

        // The Undelivered Child Reaches Its Owner, Which the Thread Drops.
        dropped.await.unwrap();
    }

    #[tokio::test]
    async fn a_panicking_request_answers_its_caller_and_the_thread_keeps_serving() {
        // One Request Panics While Taking Ownership of Its Child.
        let spawner = Spawner::new();
        let error = spawner.spawn(trivial(), |_child| -> () { panic!("Story Panic") }).await.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::Other, "Answer the Caller With an Error: {error}");

        // The Same Thread Serves the Next Request.
        let mut child = spawner.spawn(trivial(), |child| child).await.unwrap();
        assert!(child.wait().await.unwrap().success());
    }

    #[tokio::test]
    async fn a_dead_spawner_thread_is_restarted_by_the_next_spawn() {
        // A Spawner Whose Thread Has Ended Holds a Sender Nobody Receives.
        let spawner = Spawner::new();
        *spawner.requests.lock().unwrap() = Some(std::sync::mpsc::channel().0);

        // The Next Spawn Starts a New Thread and Succeeds.
        let mut child = spawner.spawn(trivial(), |child| child).await.unwrap();
        assert!(child.wait().await.unwrap().success());
    }
}
