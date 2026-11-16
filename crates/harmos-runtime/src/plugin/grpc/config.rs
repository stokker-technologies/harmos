/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// gRPC Plugin Placement Policy

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{num::NonZeroU32, time::Duration};

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Placement Policy Uses The Same Clock As The Owning Host.
#[derive(Clone, Debug)]
pub struct PlacementSettings {
    pub reconnect_delay: Duration,
    pub stop_grace: Duration,
    /// Maximum Retained Rows per Stream; The Consumer Owns This Budget.
    pub stream_window: std::num::NonZeroUsize,
    /// Interval Window Length: Longer Windows Batch More Samples per Message, Shorter Ones Deliver Sooner.
    /// A Stream's `window_setting` Overrides It.
    pub interval_window: Duration,
    /// How Far an Interval Window May Fall Behind Its End Before Its Demand Fails.
    pub interval_lag: IntervalLag,
}

/// Runtime Configuration of gRPC Plugins, Like `StorageSettings`: the Policies of Their Operations.
#[derive(Clone, Copy, Debug, Default)]
pub struct GrpcPluginSettings {
    pub policy: GrpcPluginPolicy,
}

/// The Policies of the gRPC Plugin Operations; a Direct Caller May Still Override One Call.
#[derive(Clone, Copy, Debug)]
pub struct GrpcPluginPolicy {
    /// Loading and Initializing One Plugin (`plugins.load`): Its Timeout Is the Plugin's Startup Deadline.
    pub load: crate::operations::OperationPolicy,
}

/// The Lag an Interval Demand Tolerates Past a Window's End, in Lengths of That Stream's Window.
///
/// A host that cannot keep up with a stream's declared rate grants ever longer windows; once one
/// would end more than this many window lengths late, the demand fails instead of growing further.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntervalLag(NonZeroU32);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Default for GrpcPluginPolicy {
    /// The Declared Policy of Each Operation.
    fn default() -> Self {
        Self {
            load: <super::Load as crate::operations::NativeOperation>::POLICY,
        }
    }
}

impl IntervalLag {
    pub const fn new(windows: NonZeroU32) -> Self {
        Self(windows)
    }

    pub const fn windows(self) -> NonZeroU32 {
        self.0
    }

    /// The Tolerated Lag of a Stream Whose Window Lasts `window`.
    pub fn of(self, window: Duration) -> Duration {
        window.saturating_mul(self.0.get())
    }
}

impl Default for IntervalLag {
    /// Ten Window Lengths: Clock Jitter Passes, a Host Falling Behind Fails Within a Few Windows.
    fn default() -> Self {
        Self(NonZeroU32::new(10).expect("positive default lag"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
