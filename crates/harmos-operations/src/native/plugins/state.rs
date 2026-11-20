/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Calling Entry State Operations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Access, POLICY, failure};
use crate::execution::operations::access::CallingPlugin;
use crate::execution::operations::{ExecutionContext, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_protocol::host::plugins;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Get;
pub struct Put;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Get {
    type Request = plugins::StateGetRequest;
    type Response = plugins::StateGetResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.host.v1.plugins.State/Get";
    const POLICY: crate::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Private;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Access>()];
    const REPLAY: crate::operations::ReplaySafety = crate::operations::ReplaySafety::Idempotent;
    fn grant(_: &Self::Request) -> Result<Option<crate::plugin::Grant>, crate::MessageError> {
        Ok(None)
    }
    async fn run(context: ExecutionContext, _request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let name = caller(&context)?;
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        access.0.state_get(name).await.map_err(failure)
    }
}

impl ProtobufOperation for Put {
    type Request = plugins::StatePutRequest;
    type Response = plugins::StatePutResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.host.v1.plugins.State/Put";
    const POLICY: crate::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Private;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Access>()];
    const REPLAY: crate::operations::ReplaySafety = crate::operations::ReplaySafety::Unknown;
    fn grant(_: &Self::Request) -> Result<Option<crate::plugin::Grant>, crate::MessageError> {
        Ok(None)
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let name = caller(&context)?;
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        access.0.state_put(name, request).await.map_err(failure)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn caller(context: &ExecutionContext) -> Result<String, ServingError> {
    context.resource::<CallingPlugin>().ok().and_then(|caller| caller.name.clone()).ok_or_else(|| {
        ServingError::Handler(crate::Failure::new(
            crate::operations::FailureKind::Unauthorized,
            "Access Plugin State: load this plugin as a persisted entry",
        ))
    })
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::operations::{CallContext, ResourceSet};
    use std::sync::Arc;

    #[test]
    fn state_identity_requires_the_trusted_calling_entry() {
        assert!(caller(&ExecutionContext::default()).is_err());
        let mut resources = ResourceSet::new();
        resources.insert(Arc::new(CallingPlugin { name: None })).unwrap();
        assert!(caller(&ExecutionContext::new(resources, CallContext::default())).is_err());
        let mut resources = ResourceSet::new();
        resources.insert(Arc::new(CallingPlugin { name: Some("example".into()) })).unwrap();
        assert_eq!(caller(&ExecutionContext::new(resources, CallContext::default())).unwrap(), "example");
    }
}
