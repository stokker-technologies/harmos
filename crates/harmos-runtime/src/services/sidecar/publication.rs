/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use super::*;

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
