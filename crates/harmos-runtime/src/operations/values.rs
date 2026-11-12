/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Product-Owned Value Resolution During a Plugin Invocation.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

use super::{CallContext, HostContext, OperationManager, Resource};
use harmos_operations::execution::operations::{ExecutionContext, ProtobufOperation, ServingError, Visibility};
use harmos_protocol::{
    context::{HostValuesGetRequest, HostValuesGetResponse},
    v1::SettingDeclaration,
};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub type Resolution<'a> = Pin<Box<dyn Future<Output = crate::Result<HostValuesGetResponse>> + Send + 'a>>;

pub struct Request<'a> {
    pub plugin: &'a str,
    pub declaration: &'a SettingDeclaration,
    pub context: &'a CallContext,
}

pub trait Resolver: Send + Sync + 'static {
    fn get<'a>(&'a self, request: Request<'a>) -> Resolution<'a>;
}

/// The Product Owns This Registration and Its Resolver; Dropping It Removes Access.
#[must_use]
pub struct Registration {
    registry: Weak<super::manager::Registry>,
    resolver: Arc<dyn Resolver>,
}

#[derive(Clone)]
pub(crate) struct Caller {
    pub plugin: String,
    pub declarations: Vec<SettingDeclaration>,
}

pub(crate) struct Get;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl OperationManager {
    pub fn register_values(&self, resolver: Arc<dyn Resolver>) -> crate::Result<Registration> {
        let manager = self.execution();
        let mut registered = manager.registry.values.write().expect("value resolver lock is healthy");
        if registered.is_some() {
            return Err(crate::Error::InvalidInput {
                field: "HostValues resolver",
                reason: "Get already has a registered resolver",
            });
        }
        *registered = Some(resolver.clone());
        Ok(Registration {
            registry: Arc::downgrade(&manager.registry),
            resolver,
        })
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            let mut registered = registry.values.write().expect("value resolver lock is healthy");
            if registered.as_ref().is_some_and(|current| Arc::ptr_eq(current, &self.resolver)) {
                *registered = None;
            }
        }
    }
}

impl ProtobufOperation for Get {
    type Request = HostValuesGetRequest;
    type Response = HostValuesGetResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.HostValues/Get";
    const POLICY: harmos_operations::OperationPolicy = harmos_operations::lifecycle::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<HostContext>(), Resource::of::<Caller>()];

    async fn run(execution: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let host = execution.resource::<HostContext>().map_err(|_| ServingError::Catalog)?;
        let context = host.borrow();
        let denied = || {
            ServingError::Handler(
                harmos_protocol::v1::Failure {
                    classification: harmos_protocol::v1::Classification::Refused as i32,
                    code: "value_refused".into(),
                    message: format!("Get HostValues {}: Value Is Not Available", request.name),
                    action: "Declare the Value and Register Its Resolver".into(),
                    ..Default::default()
                }
                .into(),
            )
        };
        let caller = execution.resource::<Caller>().map_err(|_| denied())?;
        let declaration = caller.declarations.iter().find(|value| value.name == request.name).ok_or_else(denied)?;
        let resolver = context.operations.registry.values.read().expect("value resolver lock is healthy").clone().ok_or_else(denied)?;
        let value = resolver
            .get(Request {
                plugin: &caller.plugin,
                declaration,
                context: context.context(),
            })
            .await
            .map_err(|source| {
                ServingError::Handler(
                    harmos_operations::Failure::originate(
                        harmos_operations::execution::operations::FailureKind::Permanent,
                        &source,
                        harmos_operations::failure::Origin::Host,
                    )
                    .in_operation(Self::ID),
                )
            })?;
        if value.r#type != declaration.r#type {
            return Err(denied());
        }
        Ok(value)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
