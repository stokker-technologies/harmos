/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Reference Product Wrappers

/* === Modules === */

pub mod client {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdk/rust/tests/fixtures/client/resources.rs"));
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{sync::OnceLock, time::Duration};

// Third Party Imports
use prost_reflect::{DescriptorPool, ServiceDescriptor};

// Local Imports
use harmos_operations::execution::operations::{
    Catalog, ExecutionContext, Grpc, OperationFactory, ProtobufOperation, Resource, ServingError, Visibility,
};
use harmos_operations::plugin::Grant;
use harmos_operations::{MessageError, OperationPolicy};

/* === Constants === */

/* === Macros === */

// The Reference Product Owns Its Remote Actions and Explicit Audience Metadata.
macro_rules! wrapper {
    ($name:ident, $handler:ty, $package:literal, $service:literal, $method:literal) => {
        pub struct $name;

        impl ProtobufOperation for $name {
            type Request = <$handler as ProtobufOperation>::Request;
            type Response = <$handler as ProtobufOperation>::Response;
            type Endpoints = <$handler as ProtobufOperation>::Endpoints;
            const ID: &'static str = concat!($package, ".", $service, "/", $method);
            const GRPC: Option<Grpc> = Some(Grpc::new($package, $service, $method));
            const POLICY: OperationPolicy = <$handler as ProtobufOperation>::POLICY;
            const VISIBILITY: Visibility = Visibility::Public;
            const RESOURCES: &'static [Resource] = <$handler as ProtobufOperation>::RESOURCES;
            const REPLAY: harmos_operations::operations::ReplaySafety = <$handler as ProtobufOperation>::REPLAY;

            fn service() -> Option<ServiceDescriptor> {
                pool().get_service_by_name(concat!($package, ".", $service))
            }

            fn grant(request: &Self::Request) -> Result<Option<Grant>, MessageError> {
                let mut grant = <$handler as ProtobufOperation>::grant(request)?;
                // The Reference Product Exposes Administrative Settings With Its Plugin Grant.
                if Self::ID == "application.host.v1.plugins.Settings/List" {
                    grant = Some(Grant::of(Self::ID, "plugins"));
                }
                if let Some(grant) = &mut grant {
                    grant.operation = Self::ID.into();
                }
                Ok(grant)
            }

            fn admitted_wait(request: &Self::Request) -> Option<Duration> {
                <$handler as ProtobufOperation>::admitted_wait(request)
            }

            async fn run(
                context: ExecutionContext, request: Self::Request, endpoints: &Self::Endpoints,
            ) -> Result<Self::Response, ServingError> {
                <$handler as ProtobufOperation>::run(context, request, endpoints).await
            }
        }
    };
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

wrapper!(TimeGet, harmos_operations::clock::Get, "application.host.v1", "Time", "Get");
wrapper!(TimeSleep, harmos_operations::clock::Sleep, "application.host.v1", "Time", "Sleep");
wrapper!(OperationsList, harmos_operations::host::List, "application.host.v1", "Operations", "List");
wrapper!(OperationsGet, harmos_operations::host::Get, "application.host.v1", "Operations", "Get");
wrapper!(SchemasList, harmos_operations::host::Schemas, "application.host.v1", "Schemas", "List");
wrapper!(StreamsList, harmos_operations::host::Streams, "application.host.v1", "Streams", "List");
wrapper!(InvocationsCreate, harmos_operations::lifecycle::Create, "application.host.v1", "Invocations", "Create");
wrapper!(InvocationsGet, harmos_operations::lifecycle::Get, "application.host.v1", "Invocations", "Get");
wrapper!(InvocationsCancel, harmos_operations::lifecycle::Cancel, "application.host.v1", "Invocations", "Cancel");
wrapper!(InvocationsWatch, harmos_operations::lifecycle::Watch, "application.host.v1", "Invocations", "Watch");
wrapper!(PluginsCreate, harmos_operations::plugins::Create, "application.host.v1", "Plugins", "Create");
wrapper!(PluginsGet, harmos_operations::plugins::Get, "application.host.v1", "Plugins", "Get");
wrapper!(PluginsList, harmos_operations::plugins::List, "application.host.v1", "Plugins", "List");
wrapper!(PluginsSet, harmos_operations::plugins::Set, "application.host.v1", "Plugins", "Set");
wrapper!(PluginsDelete, harmos_operations::plugins::Delete, "application.host.v1", "Plugins", "Delete");
wrapper!(PluginsReplace, harmos_operations::plugins::Replace, "application.host.v1", "Plugins", "Replace");
wrapper!(PluginsRestart, harmos_operations::plugins::Restart, "application.host.v1", "Plugins", "Restart");
wrapper!(PluginsWatch, harmos_operations::plugins::Watch, "application.host.v1", "Plugins", "Watch");
wrapper!(StatusGet, harmos_operations::status::Get, "application.host.v1", "Status", "Get");
wrapper!(StatusWatch, harmos_operations::status::Watch, "application.host.v1", "Status", "Watch");
wrapper!(SettingsList, harmos_operations::plugins::settings::List, "application.host.v1.plugins", "Settings", "List");
wrapper!(SettingsSet, harmos_operations::plugins::settings::Set, "application.host.v1.plugins", "Settings", "Set");
wrapper!(CertificatesCreate, harmos_operations::access::CertificatesCreate, "application.host.v1.access", "Certificates", "Create");
wrapper!(ListenerGet, harmos_operations::access::ListenerGet, "application.host.v1.access", "Listener", "Get");
wrapper!(ListenerSet, harmos_operations::access::ListenerSet, "application.host.v1.access", "Listener", "Set");
wrapper!(PairingsCreate, harmos_operations::access::PairingsCreate, "application.host.v1.access", "Pairings", "Create");
wrapper!(PairingsList, harmos_operations::access::PairingsList, "application.host.v1.access", "Pairings", "List");
wrapper!(PairingsDelete, harmos_operations::access::PairingsDelete, "application.host.v1.access", "Pairings", "Delete");
wrapper!(DiagnosticsGet, harmos_operations::status::DiagnosticsGet, "application.host.v1.status", "Diagnostics", "Get");
wrapper!(LogsList, harmos_operations::status::LogsList, "application.host.v1.status", "Logs", "List");
wrapper!(LogsWatch, harmos_operations::status::LogsWatch, "application.host.v1.status", "Logs", "Watch");
wrapper!(TraceEventsList, harmos_operations::status::TraceEventsList, "application.host.v1.status", "TraceEvents", "List");
wrapper!(TraceEventsWatch, harmos_operations::status::TraceEventsWatch, "application.host.v1.status", "TraceEvents", "Watch");

/* === Public Functions === */

pub fn catalog() -> Catalog {
    let mut catalog = Catalog::new();
    catalog.operations.push(OperationFactory::protobuf::<TimeGet>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<TimeSleep>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<OperationsList>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<OperationsGet>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<SchemasList>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<StreamsList>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<InvocationsCreate>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<InvocationsGet>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<InvocationsCancel>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<InvocationsWatch>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<StatusGet>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<StatusWatch>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<CertificatesCreate>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<ListenerGet>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<ListenerSet>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<PairingsCreate>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<PairingsList>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<PairingsDelete>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<DiagnosticsGet>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<LogsList>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<LogsWatch>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<TraceEventsList>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<TraceEventsWatch>().expect("reference wrapper"));
    catalog
}

pub fn plugins_catalog() -> Catalog {
    let mut catalog = Catalog::new();
    catalog.operations.push(OperationFactory::protobuf::<PluginsCreate>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<PluginsGet>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<PluginsList>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<PluginsSet>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<PluginsDelete>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<PluginsReplace>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<PluginsRestart>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<PluginsWatch>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<SettingsList>().expect("reference wrapper"));
    catalog.operations.push(OperationFactory::protobuf::<SettingsSet>().expect("reference wrapper"));
    catalog
}

/* === Private Functions === */

fn pool() -> &'static DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| {
        DescriptorPool::decode(
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdk/rust/tests/fixtures/client/client-descriptors.bin")).as_slice(),
        )
        .expect("reference product descriptors")
    })
}

/* === Tests === */
