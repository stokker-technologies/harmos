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
    H: Fn(&Service<A>, Notice) + Send + Sync + 'static,
{
    type Declaration = crate::operations::streams::Publish;
}

impl<A, S, H> crate::operations::Operation for Publish<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&Service<A>, Notice) + Send + Sync + 'static,
{
    type Declaration = crate::operations::streams::Publish;
    type Resources = Supervised<A, S, H>;
    type Error = crate::Error;
    type Endpoints = ();
    type Context<'a> = crate::operations::Context<'a>;

    async fn initialize<'ctx>(resources: Arc<Self::Resources>, _: Self::Context<'ctx>) -> crate::Result<Self> {
        let endpoint = resources
            .calls
            .current()
            .map_err(|source| crate::Error::OperationFailed { operation: "host.publish".into(), source: Arc::new(source) })?;
        Ok(Self { _resources: resources, endpoint })
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut crate::operations::streams::Publish, _endpoints: &mut Self::Endpoints,
    ) -> crate::Result<crate::operations::Response<()>> {
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
    type Declaration = crate::operations::streams::Publish;
    type Resources = ();
    type Error = crate::Error;
    type Endpoints = ();
    type Context<'a> = crate::operations::Context<'a>;

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> crate::Result<Self> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut crate::operations::streams::Publish, _endpoints: &mut Self::Endpoints,
    ) -> crate::Result<crate::operations::Response<()>> {
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
