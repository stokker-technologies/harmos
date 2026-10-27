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
    id = "host.operations.watch",
    response = super::InstanceState,
    output = super::InstanceState,
    policy = super::WATCH_POLICY,
    visibility = Shared,
    grpc(package = "harmos.host.v1", service = "Operations", method = "Watch"),
)]
#[derive(Clone, Default, PartialEq)]
pub struct Watch {
    pub instance: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Watch {
    pub fn of(instance: Id) -> Self {
        Self { instance: instance.value() }
    }
}

#[cfg(feature = "runtime")]
impl Operation for Watch {
    type Error = Error;

    async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<Response<super::InstanceState>, Error> {
        let access = context.resource::<harmos_sdk::operations::instances::InstanceAccess>();
        let output = context.streams();

        let instance = access.0.owned(Id::new(self.instance)).ok_or(Error::UnknownInstance(self.instance))?;
        use harmos_domain::MemorySize;
        let mut changes = instance.subscribe();
        drop(instance);
        loop {
            let status = changes.current();
            let state = super::InstanceState::from(status);
            output.send_with(state.memory_size(), || Ok(state.clone())).await?;
            if status.is_settled() || !changes.changed().await {
                return Ok(state.into());
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
