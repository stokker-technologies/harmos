/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Built-In Declarations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{future::Future, time::Duration};

// Third Party Imports

// Local Imports
use crate::execution::operations::{
    ExecutionContext, NativeOperation, NativeOutput, ProtobufOperation, ProtobufOutput, Resource, ServingError, StreamError,
};
use crate::operations::ReplaySafety;
use crate::{MemorySize, OperationPolicy};
use prost_reflect::ReflectMessage;

/* === Constants === */

/* === Macros === */

macro_rules! unary {
    ($name:ident, $operation:path) => {
        pub struct $name;
        impl NativeOperation for $name {
            type Request = <$operation as ProtobufOperation>::Request;
            type Response = <$operation as ProtobufOperation>::Response;
            type Endpoints = ();
            const ID: &'static str = <$operation as ProtobufOperation>::ID;
            const POLICY: OperationPolicy = <$operation as ProtobufOperation>::POLICY;
            const REPLAY: ReplaySafety = <$operation as ProtobufOperation>::REPLAY;
            const RESOURCES: &'static [Resource] = <$operation as ProtobufOperation>::RESOURCES;
            fn admitted_wait(request: &Self::Request) -> Option<Duration> {
                <$operation as ProtobufOperation>::admitted_wait(request)
            }
            async fn run(context: ExecutionContext, request: Self::Request, endpoints: &()) -> Result<Self::Response, ServingError> {
                <$operation as ProtobufOperation>::run(context, request, endpoints).await
            }
        }
    };
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub(crate) trait Events<T>: Sync {
    fn send(&self, value: T) -> impl Future<Output = Result<(), StreamError>> + Send;
}

/* === Implementations === */

impl<T: MemorySize + Send + Sync + 'static> Events<T> for NativeOutput<T> {
    async fn send(&self, value: T) -> Result<(), StreamError> {
        NativeOutput::send(self, value).await
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> Events<T> for ProtobufOutput<T> {
    async fn send(&self, value: T) -> Result<(), StreamError> {
        ProtobufOutput::send(self, value).await
    }
}

pub mod clock {
    use super::*;
    use crate::clock;

    unary!(Get, clock::Get);

    unary!(Sleep, clock::Sleep);
}

pub mod lifecycle {
    use super::*;
    use crate::lifecycle;
    use harmos_protocol::host::InvocationsWatchResponse;

    unary!(Cancel, lifecycle::Cancel);

    unary!(Create, lifecycle::Create);

    unary!(Get, lifecycle::Get);

    pub struct Watch;
    impl NativeOperation for Watch {
        type Request = <lifecycle::Watch as ProtobufOperation>::Request;
        type Response = <lifecycle::Watch as ProtobufOperation>::Response;
        type Endpoints = NativeOutput<InvocationsWatchResponse>;
        const ID: &'static str = <lifecycle::Watch as ProtobufOperation>::ID;
        const POLICY: OperationPolicy = <lifecycle::Watch as ProtobufOperation>::POLICY;
        const REPLAY: ReplaySafety = <lifecycle::Watch as ProtobufOperation>::REPLAY;
        const RESOURCES: &'static [Resource] = <lifecycle::Watch as ProtobufOperation>::RESOURCES;
        async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
            lifecycle::Watch::run_with(context, request, output).await
        }
    }
}

pub mod status {
    use super::*;
    use crate::status;
    use harmos_protocol::host::StatusWatchResponse;
    use harmos_protocol::host::status::LogsWatchResponse;
    use harmos_protocol::host::status::TraceEventsWatchResponse;

    unary!(DiagnosticsGet, status::DiagnosticsGet);

    unary!(Get, status::Get);

    unary!(LogsList, status::LogsList);

    pub struct LogsWatch;
    impl NativeOperation for LogsWatch {
        type Request = <status::LogsWatch as ProtobufOperation>::Request;
        type Response = <status::LogsWatch as ProtobufOperation>::Response;
        type Endpoints = NativeOutput<LogsWatchResponse>;
        const ID: &'static str = <status::LogsWatch as ProtobufOperation>::ID;
        const POLICY: OperationPolicy = <status::LogsWatch as ProtobufOperation>::POLICY;
        const REPLAY: ReplaySafety = <status::LogsWatch as ProtobufOperation>::REPLAY;
        const RESOURCES: &'static [Resource] = <status::LogsWatch as ProtobufOperation>::RESOURCES;
        async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
            status::LogsWatch::run_with(context, request, output).await
        }
    }

    unary!(TraceEventsList, status::TraceEventsList);

    pub struct TraceEventsWatch;
    impl NativeOperation for TraceEventsWatch {
        type Request = <status::TraceEventsWatch as ProtobufOperation>::Request;
        type Response = <status::TraceEventsWatch as ProtobufOperation>::Response;
        type Endpoints = NativeOutput<TraceEventsWatchResponse>;
        const ID: &'static str = <status::TraceEventsWatch as ProtobufOperation>::ID;
        const POLICY: OperationPolicy = <status::TraceEventsWatch as ProtobufOperation>::POLICY;
        const REPLAY: ReplaySafety = <status::TraceEventsWatch as ProtobufOperation>::REPLAY;
        const RESOURCES: &'static [Resource] = <status::TraceEventsWatch as ProtobufOperation>::RESOURCES;
        async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
            status::TraceEventsWatch::run_with(context, request, output).await
        }
    }

    pub struct Watch;
    impl NativeOperation for Watch {
        type Request = <status::Watch as ProtobufOperation>::Request;
        type Response = <status::Watch as ProtobufOperation>::Response;
        type Endpoints = NativeOutput<StatusWatchResponse>;
        const ID: &'static str = <status::Watch as ProtobufOperation>::ID;
        const POLICY: OperationPolicy = <status::Watch as ProtobufOperation>::POLICY;
        const REPLAY: ReplaySafety = <status::Watch as ProtobufOperation>::REPLAY;
        const RESOURCES: &'static [Resource] = <status::Watch as ProtobufOperation>::RESOURCES;
        async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
            status::Watch::run_with(context, request, output).await
        }
    }
}

pub mod access {
    use super::*;
    use crate::access;

    unary!(CertificatesCreate, access::CertificatesCreate);

    unary!(ListenerGet, access::ListenerGet);

    unary!(ListenerSet, access::ListenerSet);

    unary!(PairingsCreate, access::PairingsCreate);

    unary!(PairingsDelete, access::PairingsDelete);

    unary!(PairingsList, access::PairingsList);
}

pub mod plugins {
    use super::*;
    use crate::plugins;

    unary!(Create, plugins::Create);

    unary!(Delete, plugins::Delete);

    unary!(Get, plugins::Get);

    unary!(List, plugins::List);

    unary!(Replace, plugins::Replace);

    unary!(Restart, plugins::Restart);

    unary!(Set, plugins::Set);

    pub struct Watch;
    impl NativeOperation for Watch {
        type Request = <plugins::Watch as ProtobufOperation>::Request;
        type Response = <plugins::Watch as ProtobufOperation>::Response;
        type Endpoints = NativeOutput<Self::Response>;
        const ID: &'static str = <plugins::Watch as ProtobufOperation>::ID;
        const POLICY: OperationPolicy = <plugins::Watch as ProtobufOperation>::POLICY;
        const REPLAY: ReplaySafety = <plugins::Watch as ProtobufOperation>::REPLAY;
        const RESOURCES: &'static [Resource] = <plugins::Watch as ProtobufOperation>::RESOURCES;
        async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
            plugins::Watch::run_with(context, request, output).await
        }
    }
}

pub mod host {
    use super::*;
    use crate::host;
    unary!(List, host::List);
    unary!(Get, host::Get);
    unary!(Schemas, host::Schemas);
    unary!(Streams, host::Streams);
}

pub mod settings {
    use super::*;
    use crate::plugins::settings;

    unary!(List, settings::List);

    unary!(Set, settings::Set);
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
