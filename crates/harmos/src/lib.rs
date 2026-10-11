/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod declaration;
mod error;
#[cfg(feature = "grpc")]
pub mod grpc;
mod identity;

#[path = "storage/catalog.rs"]
mod object_catalog;

#[path = "storage/codec.rs"]
mod storage_codec;
#[path = "storage/data.rs"]
mod storage_data;
#[path = "storage/files.rs"]
mod storage_files;
#[path = "storage/frame/plan.rs"]
mod storage_frame;
#[path = "storage/mutations.rs"]
mod storage_mutations;
#[path = "storage/object_fields.rs"]
mod storage_object_fields;
#[path = "storage/object_values.rs"]
mod storage_object_values;
#[path = "storage/operations/mod.rs"]
mod storage_operations;
#[path = "storage/position.rs"]
mod storage_position;
#[path = "storage/read_selection.rs"]
mod storage_reads;
#[path = "storage/reduce/scalar.rs"]
mod storage_scalar;
#[path = "storage/schema_input.rs"]
mod storage_schema_input;
#[path = "storage/timestamp.rs"]
mod storage_timestamp;

#[cfg(feature = "runtime")]
#[path = "storage/mod.rs"]
mod storage_runtime;

pub mod storage {
    pub use crate::storage_data::{RecordInput, StoredRecord, StoredRow, StreamInput};
    pub use crate::storage_files::{
        FileMetadata, FilePhase, FileProgress, FileReference, FileSession, StagedFile,
    };
    pub use crate::storage_mutations::{FileProducer, MutationId, MutationWindow, StorageInfo};
    pub use crate::storage_object_fields::{
        ObjectComparison, ObjectFields, ObjectFilter, ObjectMetadata, ObjectNullOrder,
        ObjectScalar, ObjectScalarKind, ObjectSort,
    };
    pub use crate::storage_schema_input::{SchemaInfo, SchemaInput};
    pub mod operations {
        pub use crate::storage_operations::*;
    }
    pub mod frame {
        pub use crate::storage_frame::{
            AsOf, AsOfStrategy, Collected, Compare, Explain, Expr, Frame, GroupBy, GroupOutput,
            GroupRow, LastBy, Predicate, ScalarField, ScalarPath, Scan,
        };
        #[cfg(feature = "runtime")]
        pub use crate::storage_runtime::frame::*;
    }
    pub mod reduce {
        #[cfg(feature = "runtime")]
        pub use crate::storage_runtime::reduce::*;
        pub use crate::storage_scalar::{Agg, ExactNumber, ScalarKind, ScalarValue};
    }
    pub use crate::object_catalog::{
        ObjectCatalog, ObjectDefinition, ObjectDefinitions, ObjectKey, ObjectRetention,
        ObjectValue, PayloadEncoding, SchemaKey, StoredObject,
    };
    pub use crate::storage_object_values::{
        ObjectEntry, ObjectPoint, ObjectReceipt, ObjectRevision, ObjectVersion, ObjectVersionToken,
        ObjectVersions,
    };
    pub use crate::storage_position::{RecoveryPosition, StorageCursor, StoreHistory};
    pub use crate::storage_reads::{
        ReadHistory, ReadKind, ReadLimits, ReadOrder, ReadSelection, SourceTimeRange,
    };
    #[cfg(feature = "runtime")]
    pub use crate::storage_runtime::*;
}

#[cfg(feature = "runtime")]
mod application;

#[cfg(feature = "runtime")]
mod attachments;

#[cfg(feature = "runtime")]
mod consumer;

#[cfg(feature = "runtime")]
mod evidence;

#[cfg(feature = "runtime")]
pub mod state;

#[cfg(feature = "runtime")]
pub mod records;

#[cfg(feature = "runtime")]
#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
pub mod load;

#[cfg(all(not(target_arch = "wasm32"), feature = "plugin"))]
pub mod plugin;

#[cfg(feature = "runtime")]
#[path = "operations/mod.rs"]
mod operation_runtime;

#[cfg(feature = "runtime")]
mod runtime;

#[cfg(feature = "sim")]
pub mod sim;

#[cfg(feature = "runtime")]
pub mod streams;

#[cfg(feature = "runtime")]
pub mod services;

#[cfg(feature = "runtime")]
pub mod observers;

#[cfg(feature = "runtime")]
mod supervision;

pub mod __private {
    pub extern crate serde;
    pub extern crate serde_json;
    pub use crate::with_protobuf;
    pub use carrier::Context as CarrierContext;
    pub use harmos_contract::__private::{
        DecodeField, DecodeInput, DecodeScalar, DecodeValue, MessageError, PostcardReader,
        PostcardValue, decoding_add, decoding_multiply, decoding_vector,
    };
    #[cfg(feature = "grpc")]
    pub use harmos_contract::__private::{Protobuf, prost};
    pub use harmos_contract::inventory as PluginInventory;
    pub use harmos_contract::plugin::{
        Contribution as PluginContribution, Declared as PluginDeclared, Grant as PluginGrant,
        Requirement as PluginRequirement, Requirements as PluginRequirements,
    };
    pub use harmos_contract::{
        Registrar as PluginRegistrar, RegistrationError as PluginRegistrationError,
    };

    mod carrier {
        pub trait Context: sealed::Context {
            const FACILITY: &'static str;
        }

        mod sealed {
            pub trait Context {}
        }

        #[cfg(feature = "guest")]
        impl sealed::Context for harmos_guest::Context {}
        #[cfg(feature = "guest")]
        impl Context for harmos_guest::Context {
            const FACILITY: &'static str = "guest";
        }

        #[cfg(feature = "sidecar")]
        impl sealed::Context for harmos_sidecar::Supervisor {}
        #[cfg(feature = "sidecar")]
        impl Context for harmos_sidecar::Supervisor {
            const FACILITY: &'static str = "sidecar";
        }
    }

    // A stream declaration is authored wherever its row type is, host core or
    // no host core, so these three are as unconditional as the attribute that
    // generates them.
    pub use crate::declaration::{BatchDefinition, StreamCatalog, StreamRow};
    // How generated code names a type the tokens it was handed cannot spell.
    // Ungated on purpose: a hand-written contribution in a crate that compiles
    // no authoring attributes at all still resolves the same paths.
    #[cfg(feature = "runtime")]
    pub use crate::state::{Catalogued, Mutual};
    pub use harmos_contract::__private::Answer;

    pub fn stream_definition<S: StreamRow>(
        id: &'static str,
        version: u32,
        window: usize,
    ) -> crate::Definition {
        crate::Definition::declared::<S>(id, version, window)
            .batching(S::batch_definition())
            .presentation(S::metadata(), S::row_metadata())
    }
}

#[cfg(feature = "artifact")]
pub mod artifact {
    pub use harmos_contract::plugin;
    pub use harmos_contract::{
        ConfigError, FRAGMENT_LIMIT, KINDS, PREFIX, Plugin, RESERVED, Refusal, Registrar,
        Registration, RegistrationError, Requirement, ScanError, StreamDeclaration, UNKNOWN,
        UNREADABLE, artifact_subset, decode_config, encode_config, fragments, inventory, reserved,
        stream_subset, validate_config,
    };
}

#[cfg(any(feature = "sidecar", feature = "sidecar-host"))]
pub mod sidecar {
    pub use harmos_sidecar::operations;
    pub use harmos_sidecar::operations::{
        CacheKey, OperationDeclaration, OperationPolicy, RetryPolicy,
    };
    pub use harmos_sidecar::{
        Attach, Attached, ConfigError, Executable, ExecutableError, Frame, ObserverControl,
        ObserverExit, ObserverStatus, ObserverUpdate, Plugin, Publication, Refusal, Registrar,
        Registration, RegistrationError, StreamDeclaration, StreamSpec, inventory,
    };

    #[cfg(feature = "sidecar")]
    pub use harmos_sidecar::{
        Activation, ActivationId, CATALOG_EXPORT_FLAG, CATALOG_EXPORT_LIMIT, CallError, DemandKey,
        Instance, Observer, ObserverAttachment, ObserverCaller, ObserverContext, ObserverHost,
        ObserverPublisher, ObserverStreamPublisher, ObserverTask, PluginNative, PublicationError,
        PublishError, Publisher, Supervisor, Tick, run_plugin,
    };

    #[cfg(feature = "sidecar")]
    pub use harmos_sidecar::__private;

    #[cfg(feature = "sidecar-host")]
    pub use crate::services::{
        DemandLease, DemandSource, Managed, ManagedError, Notice, Publish, SidecarError, Supervised,
    };
}

#[cfg(any(feature = "componentize", feature = "guest", feature = "guest-host"))]
pub mod guest;

#[cfg(feature = "signal")]
pub mod signal {
    pub use harmos_signal::{
        Band, Decibel, Edge, Scale, Spectrum, Summary, Taper, above_floor, band_energy, band_rms,
        bins, centroid, crossing, decimate, derivative, edges, extrema, fundamental, harmonics,
        integral, lttb, lttb_log, magnitude, nearest, occupied_bandwidth, papr, peaks, phase,
        resample, resample_count, resample_uniform, rolloff, thd, value_at,
    };
    #[cfg(feature = "fft")]
    pub use harmos_signal::{Estimate, Welch, fft_in_place, reconstruct, transform};
}

pub mod operations {
    #[cfg(feature = "runtime")]
    pub use crate::operation_runtime::*;
    #[cfg(feature = "runtime")]
    pub use crate::operation_runtime::{StreamOutput, StreamPermit, stream};
    pub use harmos_contract::operations::*;
}

#[cfg(feature = "derive")]
pub mod macros {
    pub use harmos_macros::catalog;
}

/* === Re-Exports === */
pub use error::{BackendFailure, Error};
pub use identity::{DefinitionId, Identified};

// What an application declares about a stream, authored wherever its row type
// is.
pub use declaration::{Axial, Batched, Definition, Prefix, Track, Tracked};
#[cfg(feature = "grpc")]
pub use harmos_contract::Protobuf;
pub use harmos_contract::journal::Position;
pub use harmos_contract::plugin::Plugin;
pub use harmos_contract::{
    Identifier, IdentifierError, MemorySize, Message, MessageError, Metadata, RowMetadata,
    Storable, StreamField, StreamSpec, Value, ValueError, btree_map_memory,
};
pub use harmos_contract::{catalog, listeners, schema};

#[cfg(feature = "runtime")]
pub use operations::OperationManager;
pub use operations::{CacheKey, OperationDeclaration, OperationPolicy, RetryPolicy};

#[cfg(feature = "runtime")]
pub use state::Metadata as JournalMetadata;

#[cfg(feature = "runtime")]
pub use consumer::{Batch, Cursor};

// The fold ports an adapter implements, handed out where one is being written.
#[cfg(feature = "runtime")]
pub use consumer::{JournalConsumer, Series, StreamConsumer};

#[cfg(feature = "runtime")]
pub use evidence::{Draft, Evidence, EvidenceError, EvidenceKind, EvidenceRecord};

#[cfg(feature = "runtime")]
pub use evidence::{Excerpt, Vault};

#[cfg(feature = "runtime")]
pub use state::capture::{CaptureError, CaptureLimits, JournalCapture};

// The authoring attributes, re-exported so applications write `#[harmos::…]`
// and never name the plumbing crate. They live in the macro namespace and add
// no name to the type census.
#[cfg(feature = "runtime")]
pub use application::Application;

#[cfg(feature = "derive")]
pub use harmos_macros::{
    lanes, listener, message, observer, operation, plugin, record, records, stream, streams,
    transaction, transactions,
};

#[cfg(feature = "runtime")]
pub use records::RecordManager;

#[cfg(feature = "runtime")]
pub use state::{ManagedState, StateManager};

#[cfg(feature = "runtime")]
pub use state::{
    At, CatalogCodec, CausationId, CorrelationId, DecodeError, Entry, Envelope, FactSource, Focus,
    Invert, Kind, Link, Never, Origin, PreparationError, Prepared, PrincipalId, Receipt, Record,
    RecordCatalog, RecordProjection, Recorded, Records, RequestKey, Reversals, SystemFact,
    Timestamp, Transaction, TransactionCatalog, Undoable,
};

#[cfg(feature = "runtime")]
pub use runtime::{
    BootstrapCleanup, BootstrapStatus, Builder, Catalog, Handle, Incompatible, Inspection,
    Inspector, Registration, Runtime, Startup, StopParticipant,
};

#[cfg(feature = "sim")]
pub use sim::SimulatedRuntime;

// The decode target the journal's own catalog is written in terms of: it
// crosses at recovery, so it is the journal's vocabulary as much as an
// adapter's, and a host names it with or without one attached.
#[cfg(feature = "runtime")]
pub use attachments::Payload;

#[cfg(feature = "runtime")]
pub use attachments::{Horizon, Recovery, Source};

#[cfg(feature = "runtime")]
pub use streams::{
    Aggregate, Aggregated, BatchItem, Batches, BeneathWindow, Bucket, Demand, Item,
    ManagedPublication, Measurement, PageLimits, PluginStream, Point, PublicationLease,
    PublicationRow, Query, ReadCut, ReadError, ReadLimits, RowPage, RowReader, RowSource, Seal,
    Statistics, Stream, StreamAdmissionError, StreamFeed, StreamManager, StreamPublicationError,
    Summary, Survey,
};

#[cfg(all(
    feature = "runtime",
    any(feature = "load", feature = "guest-host", feature = "sidecar-host")
))]
pub use streams::{StagedPublication, StreamRegistration};

#[cfg(feature = "runtime")]
pub use operation_runtime::lane::{Capacity, Lane, LaneSet};
#[cfg(feature = "runtime")]
pub use operation_runtime::refusal::Refusal;
#[cfg(feature = "runtime")]
pub use operation_runtime::scope::Scope;

#[cfg(feature = "runtime")]
pub use services::{Fault, Resident, Service, ServiceExit, ServicePolicy, ServiceStatus};

/* === Imports === */
// Standard Library Imports

// Third Party Imports
extern crate self as harmos;
// Local Imports

/* === Constants === */

/* === Macros === */
// Generated And Native Message Implementations Follow The Same Facade Feature.
// Another Consumer May Independently Enable The Portable Contract's Codec.
#[cfg(feature = "grpc")]
#[macro_export]
macro_rules! with_protobuf { ($($tokens:tt)*) => { $($tokens)* }; }

#[cfg(not(feature = "grpc"))]
#[macro_export]
macro_rules! with_protobuf {
    ($($tokens:tt)*) => {};
}

/* === Type Aliases === */
pub type Result<T, E = std::convert::Infallible> = std::result::Result<T, Error<E>>;

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
