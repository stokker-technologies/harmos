// @generated from the protocol descriptor table; do not edit.
use crate::{Message, OperationRequest};
impl Message for harmos_protocol::context::CatalogEntriesListRequest {
    fn name() -> String { "harmos.context.v1.CatalogEntriesListRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.CatalogEntriesListRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::CatalogEntriesListResponse {
    fn name() -> String { "harmos.context.v1.CatalogEntriesListResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.CatalogEntriesListResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::HostValuesGetRequest {
    fn name() -> String { "harmos.context.v1.HostValuesGetRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.HostValuesGetRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::HostValuesGetResponse {
    fn name() -> String { "harmos.context.v1.HostValuesGetResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.HostValuesGetResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::InvocationsCancelRequest {
    fn name() -> String { "harmos.context.v1.InvocationsCancelRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.InvocationsCancelRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::InvocationsCancelResponse {
    fn name() -> String { "harmos.context.v1.InvocationsCancelResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.InvocationsCancelResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::InvocationsGetRequest {
    fn name() -> String { "harmos.context.v1.InvocationsGetRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.InvocationsGetRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::InvocationsGetResponse {
    fn name() -> String { "harmos.context.v1.InvocationsGetResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.InvocationsGetResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::InvocationsWatchRequest {
    fn name() -> String { "harmos.context.v1.InvocationsWatchRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.InvocationsWatchRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::InvocationsWatchResponse {
    fn name() -> String { "harmos.context.v1.InvocationsWatchResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.InvocationsWatchResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::SamplesAppendRequest {
    fn name() -> String { "harmos.context.v1.SamplesAppendRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.SamplesAppendRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::SamplesAppendResponse {
    fn name() -> String { "harmos.context.v1.SamplesAppendResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.SamplesAppendResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::TimeGetRequest {
    fn name() -> String { "harmos.context.v1.TimeGetRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.TimeGetRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::TimeGetResponse {
    fn name() -> String { "harmos.context.v1.TimeGetResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.TimeGetResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::TimeSleepRequest {
    fn name() -> String { "harmos.context.v1.TimeSleepRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.TimeSleepRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::TimeSleepResponse {
    fn name() -> String { "harmos.context.v1.TimeSleepResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.TimeSleepResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::v1::SettingDeclaration {
    fn name() -> String { "harmos.v1.SettingDeclaration".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.v1.SettingDeclaration.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::v1::SettingsDeclarations {
    fn name() -> String { "harmos.v1.SettingsDeclarations".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.v1.SettingsDeclarations.bin")[..]).expect("generated context wrapper")
    }
}
impl OperationRequest for harmos_protocol::context::CatalogEntriesListRequest {
    type Response = harmos_protocol::context::CatalogEntriesListResponse;
    const ID: &'static str = "harmos.context.v1.CatalogEntries/List";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::HostValuesGetRequest {
    type Response = harmos_protocol::context::HostValuesGetResponse;
    const ID: &'static str = "harmos.context.v1.HostValues/Get";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::InvocationsGetRequest {
    type Response = harmos_protocol::context::InvocationsGetResponse;
    const ID: &'static str = "harmos.context.v1.Invocations/Get";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::InvocationsCancelRequest {
    type Response = harmos_protocol::context::InvocationsCancelResponse;
    const ID: &'static str = "harmos.context.v1.Invocations/Cancel";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::InvocationsWatchRequest {
    type Response = harmos_protocol::context::InvocationsWatchResponse;
    const ID: &'static str = "harmos.context.v1.Invocations/Watch";
    const WATCH: bool = true;
}
impl OperationRequest for harmos_protocol::context::SamplesAppendRequest {
    type Response = harmos_protocol::context::SamplesAppendResponse;
    const ID: &'static str = "harmos.context.v1.Samples/Append";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::TimeGetRequest {
    type Response = harmos_protocol::context::TimeGetResponse;
    const ID: &'static str = "harmos.context.v1.Time/Get";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::TimeSleepRequest {
    type Response = harmos_protocol::context::TimeSleepResponse;
    const ID: &'static str = "harmos.context.v1.Time/Sleep";
    const WATCH: bool = false;
}
