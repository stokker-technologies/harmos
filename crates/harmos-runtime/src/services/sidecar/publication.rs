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

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<A, S, H> crate::operations::Declares for Publish<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync + 'static,
{
    type Declaration = crate::operations::streams::Publish;
}

impl<A, S, H> crate::operations::Operation for Publish<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync + 'static,
{
    type Request = <crate::operations::streams::Publish as crate::operations::OperationDeclaration>::Request;
    type Response = <crate::operations::streams::Publish as crate::operations::OperationDeclaration>::Response;
    type Error = crate::Error;
    type Endpoints = ();

    async fn initialize(context: harmos_sdk::OperationContext) -> crate::Result<Self> {
        let resources = context.dependency::<Supervised<A, S, H>>()?;
        let endpoint = resources
            .calls
            .current()
            .map_err(|source| crate::Error::OperationFailed { operation: "host.publish".into(), source: Arc::new(source) })?;
        Ok(Self { _resources: resources, endpoint })
    }

    async fn run(
        &mut self, context: harmos_sdk::OperationContext, request: &mut Self::Request, _endpoints: &mut Self::Endpoints,
    ) -> crate::Result<crate::operations::Response<()>> {
        let host = context.dependency::<crate::operations::HostContext>()?;
        let context = host.borrow();
        let result: ::core::result::Result<(), Self::Error> = async move {
            let scope = context
                .caller_scope()
                .ok_or(crate::Error::InvalidInput { field: "native publication", reason: "the original plugin caller is required" })?;
            self.endpoint.publish(scope, request, &context).await
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl crate::operations::Declares for HostPublish {
    type Declaration = crate::operations::streams::Publish;
}

// The host-wide `host.publish` provider: each call reaches the caller generation's live native endpoint.
impl crate::operations::Operation for HostPublish {
    type Request = <crate::operations::streams::Publish as crate::operations::OperationDeclaration>::Request;
    type Response = <crate::operations::streams::Publish as crate::operations::OperationDeclaration>::Response;
    type Error = crate::Error;
    type Endpoints = ();

    async fn initialize(_: harmos_sdk::OperationContext) -> crate::Result<Self> {
        Ok(Self)
    }

    async fn run(
        &mut self, context: harmos_sdk::OperationContext, request: &mut Self::Request, _endpoints: &mut Self::Endpoints,
    ) -> crate::Result<crate::operations::Response<()>> {
        let host = context.dependency::<crate::operations::HostContext>()?;
        let context = host.borrow();
        let scope = context
            .caller_scope()
            .ok_or(crate::Error::InvalidInput { field: "native publication", reason: "the original plugin caller is required" })?;
        let endpoint = scope.native().ok_or(crate::Error::NotReady { domain: "native publication endpoint" })?;
        endpoint.publish(scope, request, &context).await.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
