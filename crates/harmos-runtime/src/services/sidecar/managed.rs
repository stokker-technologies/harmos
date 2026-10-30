/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::io::Read;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::task::Poll;
// Third Party Imports
use sha2::{Digest, Sha256};
// Local Imports
use super::{
    Notice, Supervised,
    physical::{Physical, Polling},
};
use crate::operations::{EncodedInvocation, PluginRegistration, PluginSdkRegistration};
use crate::plugin::{
    PluginArtifact, PluginBackend, PluginDefinition, PluginDefinitions, PluginLifecycle, PluginLifecycleAuthority, PluginManager,
    PluginTransition, Router,
};
use crate::services::Resident;
use crate::{Application, Error, Result, operations::Lane, operations::Manager, services::ServiceContext};
use harmos_sidecar::{Attach, Executable};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type NativeCatalog<L> = fn(&mut crate::operations::Bindings<'_>, L, std::sync::Arc<crate::operations::Native>, NonZeroUsize) -> Result<()>;

type Diagnostics<A> = fn(&ServiceContext<A>, Notice);

type StreamListeners<A> = fn(&PluginDefinition, &mut crate::streams::PluginListenerBindings<'_, A>) -> Result<()>;

/* === Structs === */

pub(crate) struct RuntimeDriver<A: Application, L> {
    host: Host<A, Executable, Diagnostics<A>, L>,
    backend: PluginBackend<Backend<A, Executable, Diagnostics<A>>, ()>,
}

pub struct Managed<A: Application, S, H, L> {
    residents: BTreeMap<String, Supervised<A, S, H>>,
    lane: L,
    capacity: NonZeroUsize,
    encoding_memory_bytes: NonZeroUsize,
    catalog: NativeCatalog<L>,
    stream_listeners: StreamListeners<A>,
    verify_artifact: fn(&S, &PluginDefinition) -> Result<()>,
    manager: PluginManager<Backend<A, S, H>, ()>,
    host: Option<Host<A, S, H, L>>,
}

struct Backend<A: Application, S, H> {
    progress: Option<crate::plugin::Lease>,
    polling: Vec<(String, Polling<A, S, H>)>,
    exit: Option<std::result::Result<(), String>>,
    grace: std::time::Duration,
    dependencies: BTreeSet<String>,
    physical: Option<Physical<A, S, H>>,
    registration: Option<PluginRegistration>,
    sdk: Option<PluginSdkRegistration>,
    initializing: Option<EncodedInvocation>,
    shutting_down: Option<EncodedInvocation>,
    initialized: Option<tokio::sync::watch::Receiver<bool>>,
    shutdown_attempted: bool,
    stream_listeners: Option<crate::streams::PluginListenerRegistration>,
}

struct Host<A: Application, S, H, L> {
    progress: Option<crate::plugin::Driving>,
    residents: BTreeMap<String, Supervised<A, S, H>>,
    streams: crate::streams::StreamManager<A>,
    polling: Vec<(String, Polling<A, S, H>)>,
    dependencies: BTreeMap<String, BTreeSet<String>>,
    operations: Manager,
    lane: L,
    capacity: NonZeroUsize,
    encoding_memory_bytes: NonZeroUsize,
    catalog: NativeCatalog<L>,
    stream_listeners: StreamListeners<A>,
    verify_artifact: fn(&S, &PluginDefinition) -> Result<()>,
    exit: Option<std::result::Result<(), String>>,
}

/* === Enums === */

enum Progress {
    Admission,
    Settlement,
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedError {
    #[error("{0}")]
    Physical(String),

    #[error(transparent)]
    Lifecycle(#[from] Error),
}

/* === Traits === */

/* === Implementations === */

impl<A, L> RuntimeDriver<A, L>
where
    A: Application,
    L: Lane + Clone,
{
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new<C>(
        resident: Supervised<A, Executable, Diagnostics<A>>, definition: &PluginDefinition, operations: Manager,
        streams: crate::streams::StreamManager<A>, lane: L, capacity: NonZeroUsize, encoding_memory_bytes: NonZeroUsize,
        progress: crate::plugin::Driving,
    ) -> Result<Self>
    where
        C: crate::catalog::Catalog<crate::operations::Declarations> + crate::catalog::Catalog<crate::operations::NativeBindings>,
    {
        verify_executable_inputs(&resident.attach, definition)?;
        let mut host = Host {
            progress: Some(progress),
            residents: BTreeMap::from([(definition.id.clone(), resident)]),
            streams,
            polling: Vec::new(),
            dependencies: BTreeMap::from([(definition.id.clone(), BTreeSet::new())]),
            operations,
            lane,
            capacity,
            encoding_memory_bytes,
            catalog: |bindings, lane, native, capacity| bindings.register_native_catalog::<C>(lane, native, capacity),
            stream_listeners: |_, _| Ok(()),
            verify_artifact: verify_executable_artifact,
            exit: None,
        };
        let backend = host.prepare(definition)?;
        Ok(Self { host, backend })
    }

    pub(crate) async fn start(
        &mut self, definition: &PluginDefinition, router: &Router, authority: &PluginLifecycleAuthority<'_>,
    ) -> Result<()> {
        if let Err(error) = self.host.start(definition, router, authority, &mut self.backend).await {
            return match self.host.exit.take() {
                Some(Err(source)) => Err(Error::OperationFailed {
                    operation: definition.id.clone(),
                    source: Arc::new(std::io::Error::other(source)),
                }),
                Some(Ok(())) => Err(Error::NotReady { domain: "native startup exited" }),
                None => Err(error),
            };
        }
        Ok(())
    }

    pub(crate) async fn stop(&mut self, definition: &PluginDefinition, authority: &PluginLifecycleAuthority<'_>) -> Result<()> {
        Host::<A, Executable, Diagnostics<A>, L>::stop(definition, authority, &mut self.backend).await
    }

    pub(crate) fn poll(&mut self, context: &mut std::task::Context<'_>) -> Poll<Result<()>> {
        let PluginBackend::Sidecar(backend) = &mut self.backend else { unreachable!() };
        let Some(physical) = &mut backend.physical else {
            return Poll::Pending;
        };
        physical.poll(context).map(|outcome| {
            outcome.map_err(|source| Error::OperationFailed {
                operation: self.host.residents.keys().next().expect("native driver owns one plugin").clone(),
                source: Arc::new(std::io::Error::other(source)),
            })
        })
    }
}

impl<A, S, H, L> Managed<A, S, H, L>
where
    A: Application,
    S: Attach + Clone,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync + 'static,
    L: Lane + Clone,
{
    pub fn new(
        resident: Supervised<A, S, H>, definition: PluginDefinition, lane: L, capacity: NonZeroUsize, encoding_memory_bytes: NonZeroUsize,
    ) -> Result<Self> {
        let definitions = PluginDefinitions::new([definition], [])?;
        Self::new_graph([resident], definitions, lane, capacity, encoding_memory_bytes)
    }

    pub fn new_graph(
        residents: impl IntoIterator<Item = Supervised<A, S, H>>, definitions: PluginDefinitions, lane: L, capacity: NonZeroUsize,
        encoding_memory_bytes: NonZeroUsize,
    ) -> Result<Self> {
        let owners: BTreeMap<_, _> = definitions.definitions().map(|definition| (definition.id.clone(), definition.kind())).collect();
        if owners.is_empty() || owners.values().any(|kind| *kind != crate::plugin::PluginKind::Sidecar) {
            return Err(Error::InvalidInput {
                field: "managed native graph",
                reason: "graph must contain native sidecar definitions",
            });
        }
        let mut attachments = BTreeMap::new();
        for resident in residents {
            let owner = resident.attach.owner().to_owned();
            if !owners.contains_key(&owner) || attachments.contains_key(&owner) {
                return Err(Error::InvalidInput {
                    field: "managed native graph",
                    reason: "attachment must select one unique frozen sidecar owner",
                });
            }
            attachments.insert(owner, resident);
        }
        if attachments.len() != owners.len() {
            return Err(Error::InvalidInput {
                field: "managed native graph",
                reason: "every frozen sidecar owner requires an attachment",
            });
        }
        let manager = PluginManager::new(definitions)?;
        Ok(Self {
            residents: attachments,
            lane,
            capacity,
            encoding_memory_bytes,
            catalog: |bindings, lane, native, capacity| bindings.register_native(lane, native, capacity),
            stream_listeners: |_, _| Ok(()),
            verify_artifact: verify_local_artifact::<S>,
            manager,
            host: None,
        })
    }

    pub fn stream_listeners(mut self, configure: StreamListeners<A>) -> Self {
        self.stream_listeners = configure;
        self
    }

    pub fn new_catalog<C>(
        resident: Supervised<A, S, H>, definition: PluginDefinition, lane: L, capacity: NonZeroUsize, encoding_memory_bytes: NonZeroUsize,
    ) -> Result<Self>
    where
        C: crate::catalog::Catalog<crate::operations::Declarations> + crate::catalog::Catalog<crate::operations::NativeBindings>,
    {
        let mut managed = Self::new(resident, definition, lane, capacity, encoding_memory_bytes)?;
        managed.catalog = |bindings, lane, native, capacity| bindings.register_native_catalog::<C>(lane, native, capacity);
        Ok(managed)
    }
}

impl<A, H, L> Managed<A, Executable, H, L>
where
    A: Application,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync + 'static,
    L: Lane + Clone,
{
    pub fn new_executable(
        resident: Supervised<A, Executable, H>, definition: PluginDefinition, lane: L, capacity: NonZeroUsize,
        encoding_memory_bytes: NonZeroUsize,
    ) -> Result<Self> {
        verify_executable_inputs(&resident.attach, &definition)?;
        let mut managed = Self::new(resident, definition, lane, capacity, encoding_memory_bytes)?;
        managed.verify_artifact = verify_executable_artifact;
        Ok(managed)
    }
}

impl<A, S, H, L> Resident<A> for Managed<A, S, H, L>
where
    A: Application,
    S: Attach + Clone,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync + 'static,
    L: Lane + Clone,
{
    type Error = ManagedError;
    async fn run(&mut self, context: ServiceContext<A>) -> std::result::Result<(), ManagedError> {
        if context.is_stopping() {
            return Ok(());
        }
        if self.manager.statuses().any(|status| status.generation.is_some()) {
            return Err(ManagedError::Lifecycle(Error::InvalidInput {
                field: "managed native attempt",
                reason: "original generation retirement is still pending",
            }));
        }
        // Borrow Only The Original Manager's Resolved Dependency Facts.
        let mut dependencies = BTreeMap::new();
        for definition in self.manager.definitions() {
            let mut selected = BTreeSet::new();
            let mut pending = self.manager.dependencies(&definition.id).expect("frozen owner is in the original graph");
            while let Some(owner) = pending.pop() {
                if !selected.insert(owner.clone()) {
                    continue;
                }
                pending.extend(self.manager.dependencies(&owner).expect("resolved dependency is in the original graph"));
            }
            dependencies.insert(definition.id.clone(), selected);
        }
        let operations = context.operations.clone();
        self.host = Some(Host {
            progress: None,
            residents: self.residents.clone(),
            streams: context.streams().clone(),
            polling: Vec::with_capacity(self.residents.len()),
            dependencies,
            operations,
            lane: self.lane,
            capacity: self.capacity,
            encoding_memory_bytes: self.encoding_memory_bytes,
            catalog: self.catalog,
            stream_listeners: self.stream_listeners,
            verify_artifact: self.verify_artifact,
            exit: None,
        });
        let host = self.host.as_mut().expect("original service context retained");
        if let Err(error) = self.manager.initialize(host).await {
            if host.residents.values().any(|resident| resident.calls.announced_exit.load(std::sync::atomic::Ordering::Acquire)) {
                context.deactivation.store(true, std::sync::atomic::Ordering::Release);
            }
            return match host.exit.take() {
                Some(Err(fault)) => Err(ManagedError::Physical(fault)),
                _ => Err(ManagedError::Lifecycle(error)),
            };
        }
        let mut stopping = context.clone();
        let readiness = async {
            for resident in host.residents.values() {
                let mut ready = resident.ready();
                while !*ready.borrow_and_update() {
                    ready.changed().await.map_err(|_| "native readiness channel closed".to_owned())?;
                }
            }
            Ok::<_, String>(())
        };
        let mut readiness = std::pin::pin!(readiness);
        let mut ready = false;
        let running = std::future::poll_fn(|task| {
            for (_, physical) in &host.polling {
                if let Poll::Ready(outcome) = physical.poll(task) {
                    return Poll::Ready(outcome);
                }
            }
            if !ready && let Poll::Ready(outcome) = readiness.as_mut().poll(task) {
                if let Err(fault) = outcome {
                    return Poll::Ready(Err(fault));
                }
                context.ready();
                ready = true;
            }
            Poll::Pending
        });
        let outcome = stopping.until_stopping(running).await.unwrap_or(Ok(()));
        if host.residents.values().any(|resident| resident.calls.announced_exit.load(std::sync::atomic::Ordering::Acquire)) {
            context.deactivation.store(true, std::sync::atomic::Ordering::Release);
        }
        outcome.map_err(ManagedError::Physical)
    }

    async fn stop(&mut self) -> std::result::Result<(), ManagedError> {
        let Some(host) = &mut self.host else {
            return Ok(());
        };
        self.manager.shutdown(host).await.map_err(ManagedError::Lifecycle)
    }

    async fn cleanup(&mut self) -> std::result::Result<(), ManagedError> {
        self.host.take();
        Ok(())
    }
}

impl<A, S, H, L> PluginLifecycle for Host<A, S, H, L>
where
    A: Application,
    S: Attach + Clone,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync + 'static,
    L: Lane + Clone,
{
    type Sidecar = Backend<A, S, H>;
    type Embedded = ();
    fn preflight(&mut self, candidate: &PluginDefinitions) -> Result<()> {
        self.operations.validate_requirements(candidate)
    }

    fn prepare(&mut self, definition: &PluginDefinition) -> Result<PluginBackend<Self::Sidecar, ()>> {
        (self.verify_artifact)(&self.residents[&definition.id].attach, definition)?;
        Ok(PluginBackend::Sidecar(Backend {
            progress: None,
            polling: Vec::new(),
            exit: None,
            grace: self.residents[&definition.id].grace,
            dependencies: self.dependencies.remove(&definition.id).expect("original resolved dependencies transfer once"),
            physical: None,
            registration: None,
            sdk: None,
            initializing: None,
            shutting_down: None,
            initialized: None,
            shutdown_attempted: false,
            stream_listeners: None,
        }))
    }

    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }

    async fn start(
        &mut self, definition: &PluginDefinition, router: &Router, authority: &PluginLifecycleAuthority<'_>,
        backend: &mut PluginBackend<Self::Sidecar, ()>,
    ) -> Result<()> {
        let PluginBackend::Sidecar(backend) = backend else { unreachable!() };
        let original = &self.residents[&definition.id];
        let mut resident = original.clone().routed(router);
        // The Original Manager Acknowledges SDK Shutdown Before Physical Stop.
        resident.manager_owned_stop = true;
        backend.physical = Some(Physical::new(resident, &self.operations)?);
        let physical = backend.physical.as_mut().expect("physical owner retained before await");
        if let Some(progress) = &self.progress {
            let polling = physical.polling();
            backend.progress = Some(progress.retain(definition.id.clone(), authority.generation(), move |context| {
                let _ = polling.poll(context);
            }));
        }
        self.polling.push((definition.id.clone(), physical.polling()));
        backend.polling = self.polling.iter().filter(|(owner, _)| backend.dependencies.contains(owner)).cloned().collect();
        let staging = original.staged_operations();
        let staged = async {
            staging.await.map_err(|source| Error::OperationFailed {
                operation: definition.id.clone(),
                source: Arc::new(std::io::Error::other(source)),
            })
        };
        let native = Arc::new(drive(physical, &self.polling, &backend.dependencies, &mut self.exit, staged, Progress::Admission).await?);
        backend.registration = Some(self.operations.stage_plugin(native.scope().expect("original native scope"), |bindings| {
            (self.catalog)(bindings, self.lane, native.clone(), self.capacity)
        })?);
        backend.stream_listeners =
            Some(self.streams.stage_listeners(native.scope().expect("original native scope"), &self.operations, |bindings| {
                (self.stream_listeners)(definition, bindings)
            })?);
        backend.initialized = Some(native.initialization());
        backend.sdk = Some(native.stage_sdk(&self.operations, self.lane)?);
        backend.initializing =
            Some(backend.sdk.as_ref().unwrap().submit_initialize(authority, &definition.configuration, self.encoding_memory_bytes)?);
        let initializing = backend.initializing.as_mut().unwrap();
        let receipt = async { Ok(initializing.await) };
        let result = drive(physical, &self.polling, &backend.dependencies, &mut self.exit, receipt, Progress::Admission).await?;
        backend.initializing.take();
        result?;
        physical.ready();
        Ok(())
    }

    async fn stop(
        _: &PluginDefinition, authority: &PluginLifecycleAuthority<'_>, backend: &mut PluginBackend<Self::Sidecar, ()>,
    ) -> Result<()> {
        let PluginBackend::Sidecar(backend) = backend else { unreachable!() };
        let Some(physical) = &mut backend.physical else {
            return Ok(());
        };
        if let Some(listeners) = &mut backend.stream_listeners {
            drive(physical, &backend.polling, &backend.dependencies, &mut backend.exit, listeners.shutdown(), Progress::Settlement).await?;
        }
        backend.stream_listeners.take();
        if let Some(sdk) = &backend.sdk {
            drive(physical, &backend.polling, &backend.dependencies, &mut backend.exit, sdk.settle(), Progress::Settlement).await?;
            if let Some(initializing) = &mut backend.initializing {
                let receipt = async { Ok(initializing.await) };
                let _outcome =
                    drive(physical, &backend.polling, &backend.dependencies, &mut backend.exit, receipt, Progress::Settlement).await?;
                backend.initializing.take();
            }
            let initialized = backend.initialized.as_ref().is_some_and(|original| *original.borrow());
            if physical.is_running() && initialized && !backend.shutdown_attempted {
                backend.shutting_down = Some(sdk.submit_shutdown(authority)?);
                backend.shutdown_attempted = true;
            }
            if let Some(shutdown) = &mut backend.shutting_down {
                let grace = backend.grace;
                let receipt = async { Ok((&mut *shutdown).await) };
                let orderly = crate::supervision::executor::timeout(
                    grace,
                    drive(physical, &backend.polling, &backend.dependencies, &mut backend.exit, receipt, Progress::Settlement),
                )
                .await;
                let result = match orderly {
                    Ok(result) => result?,
                    Err(_) => {
                        // End The Original Carrier, Then Join Its Retained SDK Receipt.
                        physical.request_cancel();
                        drive(
                            physical,
                            &backend.polling,
                            &backend.dependencies,
                            &mut backend.exit,
                            async { Ok(shutdown.await) },
                            Progress::Settlement,
                        )
                        .await?
                    }
                };
                backend.shutting_down.take();
                result?;
            }
        }
        if let Some(registration) = &mut backend.registration {
            drive(physical, &backend.polling, &backend.dependencies, &mut backend.exit, registration.shutdown(), Progress::Settlement)
                .await?;
        }
        physical.request_cancel();
        drive(physical, &backend.polling, &backend.dependencies, &mut backend.exit, async { Ok(()) }, Progress::Settlement).await?;
        if let Some(sdk) = &mut backend.sdk {
            sdk.close().await?;
        }
        backend.sdk.take();
        backend.registration.take();
        backend.physical.take();
        backend.progress.take();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn verify_local_artifact<S>(_: &S, definition: &PluginDefinition) -> Result<()> {
    if matches!(&definition.artifact, PluginArtifact::LocalSidecar { .. }) {
        return Ok(());
    }
    Err(Error::InvalidInput {
        field: "managed native artifact",
        reason: "process artifacts require an executable-backed constructor",
    })
}

fn verify_executable_inputs(executable: &Executable, definition: &PluginDefinition) -> Result<()> {
    let PluginArtifact::Sidecar {
        path, arguments, directory, environment, ..
    } = &definition.artifact
    else {
        return Err(Error::InvalidInput {
            field: "managed native artifact",
            reason: "executable attachment requires a frozen process artifact",
        });
    };
    let actual_environment = executable.environment().iter().cloned().collect::<std::collections::BTreeMap<_, _>>();
    if path.as_path() != executable.path()
        || arguments.as_slice() != executable.arguments()
        || directory.as_deref() != executable.working_directory()
        || environment != &actual_environment
    {
        return Err(Error::InvalidInput {
            field: "managed native artifact",
            reason: "frozen launch inputs differ from the original executable attachment",
        });
    }
    Ok(())
}

fn verify_executable_artifact(executable: &Executable, definition: &PluginDefinition) -> Result<()> {
    verify_executable_inputs(executable, definition)?;
    let PluginArtifact::Sidecar { sha256, .. } = &definition.artifact else { unreachable!() };
    let io_error = |source| Error::OperationFailed {
        operation: definition.id.clone(),
        source: Arc::new(std::io::Error::other(source)),
    };
    let mut file = std::fs::File::open(executable.path()).map_err(io_error)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let length = file.read(&mut buffer).map_err(io_error)?;
        if length == 0 {
            break;
        }
        digest.update(&buffer[..length]);
    }
    let actual: [u8; 32] = digest.finalize().into();
    if actual != *sha256 {
        return Err(Error::InvalidInput {
            field: "managed native artifact",
            reason: "the inspected executable changed before activation",
        });
    }
    Ok(())
}

fn physical_cleanup(message: String) -> Error {
    Error::TaskFailed {
        domain: "native physical cleanup",
        source: Arc::new(ManagedError::Physical(message)),
    }
}

async fn drive<A, S, H, T>(
    physical: &mut Physical<A, S, H>, polling: &[(String, Polling<A, S, H>)], dependencies: &BTreeSet<String>,
    exit: &mut Option<std::result::Result<(), String>>, future: impl Future<Output = Result<T>>, progress: Progress,
) -> Result<T>
where
    A: Application,
    S: Attach,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync + 'static,
{
    if matches!(progress, Progress::Admission) && !physical.is_running() {
        return Err(Error::Closing { domain: "native physical attempt" });
    }
    if matches!(progress, Progress::Settlement) {
        physical.retry_ending();
        for (owner, sibling) in polling {
            if dependencies.contains(owner) {
                sibling.retry_ending();
            }
        }
    }
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|context| {
        if (physical.is_running() || physical.has_outcome())
            && let Poll::Ready(outcome) = physical.poll(context)
        {
            exit.get_or_insert(outcome);
            if matches!(progress, Progress::Admission) {
                return Poll::Ready(Err(Error::Closing { domain: "native physical attempt" }));
            }
        }
        for (_, sibling) in polling {
            if !sibling.same(physical)
                && let Poll::Ready(outcome) = sibling.poll(context)
            {
                exit.get_or_insert(outcome);
                if matches!(progress, Progress::Admission) {
                    return Poll::Ready(Err(Error::Closing { domain: "native physical attempt" }));
                }
            }
        }
        if matches!(progress, Progress::Settlement) {
            let mut target_pending = false;
            if !physical.is_running() {
                match physical.poll_ending(context) {
                    Poll::Ready(Ok(())) => {}
                    Poll::Ready(Err(error)) => return Poll::Ready(Err(physical_cleanup(error))),
                    Poll::Pending => target_pending = true,
                }
            }
            // Advance Every Ending; Only Required Providers Can Fail This Receipt.
            // An Unrelated Failure Remains On Its Original Owner For Its Own Retry.
            for (owner, sibling) in polling {
                if !sibling.same(physical)
                    && let Poll::Ready(Err(error)) = sibling.poll_ending(context)
                    && dependencies.contains(owner)
                {
                    return Poll::Ready(Err(physical_cleanup(error)));
                }
            }
            // The Original Receipt Cannot Settle Before Its Own Physical Ending.
            if target_pending {
                return Poll::Pending;
            }
        }
        future.as_mut().poll(context)
    })
    .await
}

/* === Tests === */
