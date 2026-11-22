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
use std::{
    num::{NonZeroU32, NonZeroUsize},
    time::Duration,
};

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
    pub stream_window: NonZeroUsize,

    /// Interval Window Length: Longer Windows Batch More Samples per Message, Shorter Ones Deliver Sooner.
    /// A Stream's `window_setting` Overrides It.
    pub interval_window: Duration,

    /// How Far an Interval Window May Fall Behind Its End Before Skipping, and When Repeated Lag Fails.
    pub interval_lag: IntervalLag,
}

/// Runtime Configuration of gRPC Plugins, Like `StorageSettings`: the Policies of Their Operations.
#[derive(Clone, Copy, Debug, Default)]
pub struct GrpcPluginSettings {
    pub policy: GrpcPluginPolicy,

    /// The Standard-Error Tail a Managed Process Keeps Until Its Activation Settles.
    pub startup_output: StartupOutput,
}

/// Bounds the Standard-Error Tail a Failed Managed Startup Reports in `Failure.diagnostics`.
///
/// A longer line is cut at `bytes`; the oldest lines leave first once either bound is reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartupOutput {
    pub lines: NonZeroUsize,
    pub bytes: NonZeroUsize,
}

/// The Policies of the gRPC Plugin Operations; a Direct Caller May Still Override One Call.
#[derive(Clone, Copy, Debug)]
pub struct GrpcPluginPolicy {
    /// Loading and Initializing One Plugin (`plugins.load`): Its Timeout Is the Plugin's Startup Deadline.
    pub load: crate::operations::OperationPolicy,
}

/// The Lag an Interval Demand Tolerates Past a Window's End, in Lengths of That Stream's Window.
///
/// Late windows are skipped with an explicit gap. Only consecutive lagging wakes fail the demand;
/// a timely window resets that count, and one clock jump counts as one wake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntervalLag {
    windows: NonZeroU32,
    consecutive_windows: NonZeroU32,
}

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

impl Default for StartupOutput {
    /// Forty Lines Within Eight Kibibytes: a Typical Interpreter Traceback.
    fn default() -> Self {
        Self {
            lines: NonZeroUsize::new(40).expect("positive default line bound"),
            bytes: NonZeroUsize::new(8192).expect("positive default byte bound"),
        }
    }
}

impl IntervalLag {
    pub const fn new(windows: NonZeroU32, consecutive_windows: NonZeroU32) -> Self {
        Self { windows, consecutive_windows }
    }

    pub const fn windows(self) -> NonZeroU32 {
        self.windows
    }

    pub const fn consecutive_windows(self) -> NonZeroU32 {
        self.consecutive_windows
    }

    /// The Tolerated Lag of a Stream Whose Window Lasts `window`.
    pub fn of(self, window: Duration) -> Duration {
        window.saturating_mul(self.windows.get())
    }
}

impl Default for IntervalLag {
    /// Skip Beyond Ten Window Lengths; Fail After Three Consecutive Lagging Wakes.
    fn default() -> Self {
        Self::new(NonZeroU32::new(10).expect("positive default lag"), NonZeroU32::new(3).expect("positive default persistence"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
