/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === --- === Definition Module === --- === */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Duration, Echoed, OperationPolicy, RetryPolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(::harmos_guest::Message, Debug, Default)]
#[harmos_path(::harmos_guest)]
#[derive(::harmos_guest::Request)]
#[request(response = Echoed)]
#[derive(Clone, PartialEq)]

pub struct ProviderEchoOperation {
    pub say: String,
}

pub static PROVIDER_ECHO_OPERATION_DESCRIPTION: ::harmos_guest::operations::WireDeclaration =
    ::harmos_guest::operations::WireDeclaration::request::<ProviderEchoOperation>(
        "provider.echo",
        OperationPolicy {
            timeout: Duration::from_millis(100),
            attempt_timeout: Duration::from_millis(100),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    )
    .visibility(::harmos_guest::operations::Visibility::Public);

pub const PROVIDER_ECHO_OPERATION_OPERATION: ::harmos_guest::operations::OperationSpec<ProviderEchoOperation> =
    ::harmos_guest::operations::OperationSpec::<ProviderEchoOperation>::new(&PROVIDER_ECHO_OPERATION_DESCRIPTION);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
