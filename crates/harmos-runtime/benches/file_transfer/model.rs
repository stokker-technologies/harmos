/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compose a Neutral Storage Benchmark Host

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Model;
pub struct AllowTransfers;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;
    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}
impl harmos_runtime::Application for Model {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
}
impl harmos_runtime::operations::authorization::Authorizer for AllowTransfers {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        Ok(Some(harmos_runtime::operations::authorization::Authorization::Full(provider.contract.clone())))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
