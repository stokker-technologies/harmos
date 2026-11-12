/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Inspect Guest Registration

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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn inspect(bytes: &[u8]) -> Result<harmos_protocol::v1::GuestRegistration, crate::Error> {
    std::thread::scope(|threads| {
        threads
            .spawn(|| {
                let executor = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
                executor
                    .block_on(harmos_runtime::guest::inspect(bytes, harmos_runtime::config::PluginGuestSettings::default()))
                    .map_err(|error| crate::Error::Component(error.to_string()))
            })
            .join()
            .map_err(|_| crate::Error::Component("Guest Inspection Worker Panicked".into()))?
    })
}

/* === Private Functions === */

/* === Tests === */
