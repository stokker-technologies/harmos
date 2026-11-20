/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Context Facilities

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::execution::operations::{ExecutionContext, OperationFactory, ProtobufOperation, Resource, ServingError, Visibility};
use crate::operations::ReplaySafety;
use crate::plugin::Grant;
use crate::{MessageError, OperationPolicy};

/* === Constants === */

/* === Macros === */

// Separate Carrier Contracts Share the Trusted In-Process Handler Mechanics.
macro_rules! facility {
    ($name:ident, $handler:ty, $id:literal) => {
        pub struct $name;

        impl ProtobufOperation for $name {
            type Request = <$handler as ProtobufOperation>::Request;
            type Response = <$handler as ProtobufOperation>::Response;
            type Endpoints = <$handler as ProtobufOperation>::Endpoints;
            const ID: &'static str = $id;
            const POLICY: OperationPolicy = <$handler as ProtobufOperation>::POLICY;
            const VISIBILITY: Visibility = Visibility::Public;
            const REPLAY: ReplaySafety = <$handler as ProtobufOperation>::REPLAY;
            const RESOURCES: &'static [Resource] = <$handler as ProtobufOperation>::RESOURCES;

            fn grant(request: &Self::Request) -> Result<Option<Grant>, MessageError> {
                <$handler as ProtobufOperation>::grant(request)
            }

            fn admitted_wait(request: &Self::Request) -> Option<std::time::Duration> {
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

facility!(TimeGet, super::clock::Get, "harmos.plugin.v1.Time/Get");
facility!(TimeSleep, super::clock::Sleep, "harmos.plugin.v1.Time/Sleep");
facility!(InvocationsGet, super::lifecycle::Get, "harmos.plugin.v1.Invocations/Get");
facility!(InvocationsCancel, super::lifecycle::Cancel, "harmos.plugin.v1.Invocations/Cancel");
facility!(InvocationsWatch, super::lifecycle::Watch, "harmos.plugin.v1.Invocations/Watch");
facility!(OperationsList, crate::host::List, "harmos.plugin.v1.Operations/List");
facility!(OperationsGet, crate::host::Get, "harmos.plugin.v1.Operations/Get");
facility!(StateGet, super::plugins::state::Get, "harmos.plugin.v1.plugins.State/Get");
facility!(StatePut, super::plugins::state::Put, "harmos.plugin.v1.plugins.State/Put");
facility!(SettingsList, super::plugins::settings::List, "harmos.plugin.v1.plugins.Settings/List");

/* === Public Functions === */

pub fn factories() -> [Result<OperationFactory, ServingError>; 10] {
    [
        OperationFactory::protobuf::<TimeGet>(),
        OperationFactory::protobuf::<TimeSleep>(),
        OperationFactory::protobuf::<InvocationsGet>(),
        OperationFactory::protobuf::<InvocationsCancel>(),
        OperationFactory::protobuf::<InvocationsWatch>(),
        OperationFactory::protobuf::<OperationsList>(),
        OperationFactory::protobuf::<OperationsGet>(),
        OperationFactory::protobuf::<SettingsList>(),
        OperationFactory::protobuf::<StateGet>(),
        OperationFactory::protobuf::<StatePut>(),
    ]
}

/* === Private Functions === */

/* === Tests === */
