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

#[cfg(feature = "runtime")]
use harmos_sdk::operations::instances::InstanceAccess;

#[cfg(feature = "runtime")]
use crate::{Error, Operation, Response};
use harmos_domain::operations::instance::Id;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(
    harmos_path = crate,
    resources = [InstanceAccess],
    id = "host.operations.cancel",
    response = super::InstanceState,
    policy = super::POLICY,
    visibility = Shared,
    grpc(package = "harmos.host.v1", service = "Operations", method = "Cancel"),
)]
#[derive(Clone, Default, PartialEq)]
pub struct Cancel {
    pub instance: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Cancel {
    pub fn of(instance: Id) -> Self {
        Self { instance: instance.value() }
    }
}

#[cfg(feature = "runtime")]
impl Operation for Cancel {
    type Error = Error;

    async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<Response<super::InstanceState>, Error> {
        let access = context.resource::<harmos_sdk::operations::instances::InstanceAccess>();

        let instance = access.0.owned(Id::new(self.instance)).ok_or(Error::UnknownInstance(self.instance))?;
        instance.cancel();
        Ok(super::InstanceState::from(instance.status()).into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
