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
use crate::{Error, Operation, Response};
use harmos_domain::operations::instance::Id;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(harmos_path = crate, resources = [harmos_sdk::operations::instances::InstanceAccess], id = "host.operations.watch", response = super::InstanceState, output = super::InstanceState, policy = super::WATCH_POLICY, visibility = harmos_domain::operations::Visibility::Shared,
    grpc(package = "harmos.host.v1", service = "Operations", method = "Watch"))]
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

    async fn run(
        mut self, context: harmos_sdk::Context<'_, Self>,
    ) -> Result<Response<<Self as harmos_domain::OperationDeclaration>::Response>, Error> {
        let access = context.resource::<harmos_sdk::operations::instances::InstanceAccess>();
        let request = &mut self;
        let output = context.streams();

        let access = access;
        let instance = access.0.owned(Id::new(request.instance)).ok_or(Error::UnknownInstance(request.instance))?;
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
