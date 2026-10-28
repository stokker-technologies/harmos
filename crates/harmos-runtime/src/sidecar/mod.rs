/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub use harmos_macros::{plugin, sidecar_catalog as catalog, sidecar_operation as operation};

pub use harmos_sidecar::operations;
pub use harmos_sidecar::operations::{CacheKey, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy};
pub use harmos_sidecar::{
    Attach, Attached, ConfigError, Executable, ExecutableError, Frame, FramePayload, Plugin, Publication, Refusal, Registrar, Registration,
    RegistrationError, StreamDeclaration, StreamSpec, inventory,
};

pub use harmos_sidecar::{
    Activation, ActivationId, Activity, CATALOG_EXPORT_FLAG, CATALOG_EXPORT_LIMIT, CallError, DemandKey, Instance, Observer,
    ObserverAttachment, ObserverCaller, ObserverContext, ObserverError, ObserverHost, ObserverPublisher, ObserverStreamPublisher,
    PluginNative, PublicationError, PublishError, Publisher, Supervisor, Tick, run_plugin,
};

pub use harmos_sidecar::__private;

pub use crate::services::{DemandLease, DemandSource, Managed, ManagedError, Notice, SidecarError, Supervised};
