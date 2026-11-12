/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Resource Management

/* === Modules === */

mod create;
mod delete;
mod get;
mod list;
mod restart;
mod set;
mod watch;

/* === Re-Exports === */

pub use create::Create;
pub use delete::Delete;
pub use get::Get;
pub use list::List;
pub use restart::Restart;
pub use set::Set;
pub use watch::Watch;

/* === Imports === */

// Standard Library Imports
use std::{future::Future, pin::Pin, sync::Arc};

// Third Party Imports

// Local Imports
use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::{v1 as wire, v1};

/* === Constants === */

const POLICY: crate::OperationPolicy = crate::OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(30),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: crate::RetryPolicy::Never,
};
const WATCH_POLICY: crate::OperationPolicy = crate::OperationPolicy {
    timeout: crate::operations::UNBOUNDED,
    attempt_timeout: crate::operations::UNBOUNDED,
    ..POLICY
};

/* === Macros === */

/* === Type Aliases === */

pub type Pending<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

/* === Structs === */

#[derive(Clone)]
pub struct Access(pub Arc<dyn Management>);

#[derive(Debug, thiserror::Error)]
#[error("{message}: {action}", message = .0.message, action = .0.action)]
pub struct Error(pub Box<v1::Failure>);

/* === Enums === */

/* === Traits === */

pub trait Management: Send + Sync + 'static {
    fn get(&self, request: wire::PluginsGetRequest) -> Pending<'_, wire::PluginsGetResponse>;
    fn list(&self, request: wire::PluginsListRequest) -> Pending<'_, wire::PluginsListResponse>;
    fn create(&self, request: wire::PluginsCreateRequest) -> Pending<'_, wire::PluginsCreateResponse>;
    fn set(&self, request: wire::PluginsSetRequest) -> Pending<'_, wire::PluginsSetResponse>;
    fn delete(&self, request: wire::PluginsDeleteRequest) -> Pending<'_, wire::PluginsDeleteResponse>;
    fn restart(&self, request: wire::PluginsRestartRequest) -> Pending<'_, wire::PluginsRestartResponse>;
    fn watch(&self, request: wire::PluginsWatchRequest) -> Pending<'_, tokio::sync::mpsc::Receiver<wire::PluginsWatchResponse>>;
}

/* === Implementations === */

impl Error {
    pub fn new(code: &str, message: impl Into<String>, action: &str) -> Self {
        Self(Box::new(v1::Failure {
            classification: v1::Classification::Permanent as i32,
            code: code.into(),
            message: message.into(),
            action: action.into(),
            ..Default::default()
        }))
    }
}

pub fn catalog() -> crate::execution::operations::Catalog {
    use crate::execution::operations::OperationFactory;
    crate::execution::__private::Catalog {
        name: "harmos.plugins",
        operations: vec![
            OperationFactory::protobuf::<Get>().expect("plugin get declaration"),
            OperationFactory::protobuf::<List>().expect("plugin list declaration"),
            OperationFactory::protobuf::<Create>().expect("plugin create declaration"),
            OperationFactory::protobuf::<Set>().expect("plugin set declaration"),
            OperationFactory::protobuf::<Delete>().expect("plugin delete declaration"),
            OperationFactory::protobuf::<Restart>().expect("plugin restart declaration"),
            OperationFactory::protobuf::<Watch>().expect("plugin watch declaration"),
        ],
        services: vec![],
    }
}

fn admit(context: &ExecutionContext, id: &'static str) -> Result<(), ServingError> {
    let grants = context.resource::<crate::execution::operations::access::GrantAccess>().map_err(|_| ServingError::Catalog)?;
    grants
        .0
        .admit(&crate::plugin::Grant::of(id, "plugins"))
        .map_err(|error| ServingError::Handler(crate::Failure::new(crate::operations::FailureKind::Permanent, error.to_string())))
}

fn failure(error: Error) -> ServingError {
    ServingError::Handler(crate::Failure::from(*error.0))
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
