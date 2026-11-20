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
mod replace;
mod restart;
mod set;
pub mod settings;
pub mod state;
mod watch;

/* === Re-Exports === */

pub use create::Create;
pub use delete::Delete;
pub use get::Get;
pub use list::List;
pub use replace::Replace;
pub use restart::Restart;
pub use set::Set;
pub use watch::Watch;

/* === Imports === */

// Standard Library Imports
use std::{future::Future, pin::Pin, sync::Arc};

// Third Party Imports

// Local Imports
use crate::execution::operations::ServingError;
use harmos_protocol::host;
use harmos_protocol::model;

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
pub struct Error(pub Box<model::Failure>);

/* === Enums === */

/* === Traits === */

pub trait Management: Send + Sync + 'static {
    fn state_get(&self, name: String) -> Pending<'_, host::plugins::StateGetResponse>;
    fn state_put(&self, name: String, request: host::plugins::StatePutRequest) -> Pending<'_, host::plugins::StatePutResponse>;
    fn replace(&self, request: host::PluginsReplaceRequest) -> Pending<'_, host::PluginsReplaceResponse>;
    fn settings_list(&self, request: host::plugins::SettingsListRequest) -> Pending<'_, host::plugins::SettingsListResponse>;
    fn settings_set(&self, request: host::plugins::SettingsSetRequest) -> Pending<'_, host::plugins::SettingsSetResponse>;
    fn get(&self, request: host::PluginsGetRequest) -> Pending<'_, host::PluginsGetResponse>;
    fn list(&self, request: host::PluginsListRequest) -> Pending<'_, host::PluginsListResponse>;
    fn create(&self, request: host::PluginsCreateRequest) -> Pending<'_, host::PluginsCreateResponse>;
    fn set(&self, request: host::PluginsSetRequest) -> Pending<'_, host::PluginsSetResponse>;
    fn delete(&self, request: host::PluginsDeleteRequest) -> Pending<'_, host::PluginsDeleteResponse>;
    fn restart(&self, request: host::PluginsRestartRequest) -> Pending<'_, host::PluginsRestartResponse>;
    fn watch(&self, request: host::PluginsWatchRequest) -> Pending<'_, tokio::sync::mpsc::Receiver<host::PluginsWatchResponse>>;
}

/* === Implementations === */

impl Error {
    pub fn new(code: &str, message: impl Into<String>, action: &str) -> Self {
        Self(Box::new(model::Failure {
            classification: model::Classification::Permanent as i32,
            code: code.into(),
            message: message.into(),
            action: action.into(),
            ..Default::default()
        }))
    }

    /// A Failure the Caller Resolves by Retrying Once the Reported Condition Has Passed.
    pub fn transient(code: &str, message: impl Into<String>, action: &str) -> Self {
        let mut error = Self::new(code, message, action);
        error.0.classification = model::Classification::Transient as i32;
        error
    }
}

pub fn catalog() -> crate::execution::operations::Catalog {
    use crate::execution::operations::OperationFactory;
    crate::execution::__private::Catalog {
        name: "harmos.plugins",
        operations: vec![
            OperationFactory::protobuf::<state::Get>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::plugin_state::Get>)
                .expect("plugin state get declaration"),
            OperationFactory::protobuf::<state::Put>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::plugin_state::Put>)
                .expect("plugin state put declaration"),
            OperationFactory::protobuf::<Get>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::plugins::Get>)
                .expect("plugin get declaration"),
            OperationFactory::protobuf::<List>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::plugins::List>)
                .expect("plugin list declaration"),
            OperationFactory::protobuf::<Create>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::plugins::Create>)
                .expect("plugin create declaration"),
            OperationFactory::protobuf::<Set>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::plugins::Set>)
                .expect("plugin set declaration"),
            OperationFactory::protobuf::<Delete>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::plugins::Delete>)
                .expect("plugin delete declaration"),
            OperationFactory::protobuf::<Restart>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::plugins::Restart>)
                .expect("plugin restart declaration"),
            OperationFactory::protobuf::<Watch>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::plugins::Watch>)
                .expect("plugin watch declaration"),
            OperationFactory::protobuf::<Replace>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::plugins::Replace>)
                .expect("plugin replace declaration"),
            OperationFactory::protobuf::<settings::List>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::settings::List>)
                .expect("plugin settings list declaration"),
            OperationFactory::protobuf::<settings::Set>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::settings::Set>)
                .expect("plugin settings set declaration"),
        ],
        services: vec![],
    }
}

fn failure(error: Error) -> ServingError {
    ServingError::Handler(crate::Failure::from(*error.0))
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
