// @generated from the protocol descriptor table; do not edit.
use crate::{Message, OperationRequest};
impl Message for harmos_protocol::plugin::host::InvocationsCancelRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsCancelRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsCancelRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::InvocationsCancelResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsCancelResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsCancelResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::InvocationsGetRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsGetRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsGetRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::InvocationsGetResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsGetResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsGetResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::InvocationsWatchRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsWatchRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsWatchRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::InvocationsWatchResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsWatchResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsWatchResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::OperationsGetRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.OperationsGetRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.OperationsGetRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::OperationsGetResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.OperationsGetResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.OperationsGetResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::OperationsListRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.OperationsListRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.OperationsListRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::OperationsListResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.OperationsListResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.OperationsListResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::SettingsListRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.SettingsListRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.SettingsListRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::SettingsListResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.SettingsListResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.SettingsListResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::TimeGetRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.TimeGetRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.TimeGetRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::TimeGetResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.TimeGetResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.TimeGetResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::TimeSleepRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.TimeSleepRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.TimeSleepRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::plugin::host::TimeSleepResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.TimeSleepResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.plugin.host.v1.TimeSleepResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::v1::SettingDeclaration {
    fn name() -> String {
        "harmos.v1.SettingDeclaration".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.v1.SettingDeclaration.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::v1::SettingsDeclarations {
    fn name() -> String {
        "harmos.v1.SettingsDeclarations".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.v1.SettingsDeclarations.bin")[..]).expect("generated context wrapper")
    }
}
impl OperationRequest for harmos_protocol::plugin::host::InvocationsGetRequest {
    type Response = harmos_protocol::plugin::host::InvocationsGetResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Invocations/Get";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::plugin::host::InvocationsCancelRequest {
    type Response = harmos_protocol::plugin::host::InvocationsCancelResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Invocations/Cancel";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::plugin::host::InvocationsWatchRequest {
    type Response = harmos_protocol::plugin::host::InvocationsWatchResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Invocations/Watch";
    const WATCH: bool = true;
}
impl OperationRequest for harmos_protocol::plugin::host::OperationsListRequest {
    type Response = harmos_protocol::plugin::host::OperationsListResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Operations/List";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::plugin::host::OperationsGetRequest {
    type Response = harmos_protocol::plugin::host::OperationsGetResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Operations/Get";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::plugin::host::SettingsListRequest {
    type Response = harmos_protocol::plugin::host::SettingsListResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Settings/List";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::plugin::host::TimeGetRequest {
    type Response = harmos_protocol::plugin::host::TimeGetResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Time/Get";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::plugin::host::TimeSleepRequest {
    type Response = harmos_protocol::plugin::host::TimeSleepResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Time/Sleep";
    const WATCH: bool = false;
}
