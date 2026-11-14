// @generated from the protocol descriptor table; do not edit.
use crate::{Message, OperationRequest};
use harmos_protocol::plugin::host::InvocationsCancelRequest;
use harmos_protocol::plugin::host::InvocationsCancelResponse;
use harmos_protocol::plugin::host::InvocationsGetRequest;
use harmos_protocol::plugin::host::InvocationsGetResponse;
use harmos_protocol::plugin::host::InvocationsWatchRequest;
use harmos_protocol::plugin::host::InvocationsWatchResponse;
use harmos_protocol::plugin::host::OperationsGetRequest;
use harmos_protocol::plugin::host::OperationsGetResponse;
use harmos_protocol::plugin::host::OperationsListRequest;
use harmos_protocol::plugin::host::OperationsListResponse;
use harmos_protocol::plugin::host::SettingsListRequest;
use harmos_protocol::plugin::host::SettingsListResponse;
use harmos_protocol::plugin::host::TimeGetRequest;
use harmos_protocol::plugin::host::TimeGetResponse;
use harmos_protocol::plugin::host::TimeSleepRequest;
use harmos_protocol::plugin::host::TimeSleepResponse;
use harmos_protocol::v1::SettingDeclaration;
use harmos_protocol::v1::SettingsDeclarations;
use prost::Message as _;
use prost_types::FileDescriptorSet;
impl Message for InvocationsCancelRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsCancelRequest".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsCancelRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for InvocationsCancelResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsCancelResponse".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsCancelResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for InvocationsGetRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsGetRequest".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsGetRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for InvocationsGetResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsGetResponse".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsGetResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for InvocationsWatchRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsWatchRequest".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsWatchRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for InvocationsWatchResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.InvocationsWatchResponse".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.InvocationsWatchResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for OperationsGetRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.OperationsGetRequest".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.OperationsGetRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for OperationsGetResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.OperationsGetResponse".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.OperationsGetResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for OperationsListRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.OperationsListRequest".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.OperationsListRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for OperationsListResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.OperationsListResponse".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.OperationsListResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for SettingsListRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.SettingsListRequest".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.SettingsListRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for SettingsListResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.SettingsListResponse".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.SettingsListResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for TimeGetRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.TimeGetRequest".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.TimeGetRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for TimeGetResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.TimeGetResponse".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.TimeGetResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for TimeSleepRequest {
    fn name() -> String {
        "harmos.plugin.host.v1.TimeSleepRequest".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.TimeSleepRequest.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for TimeSleepResponse {
    fn name() -> String {
        "harmos.plugin.host.v1.TimeSleepResponse".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.plugin.host.v1.TimeSleepResponse.bin")[..])
            .expect("generated context wrapper")
    }
}
impl Message for SettingDeclaration {
    fn name() -> String {
        "harmos.v1.SettingDeclaration".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.v1.SettingDeclaration.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for SettingsDeclarations {
    fn name() -> String {
        "harmos.v1.SettingsDeclarations".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet::decode(&include_bytes!("types/harmos.v1.SettingsDeclarations.bin")[..]).expect("generated context wrapper")
    }
}
impl OperationRequest for InvocationsGetRequest {
    type Response = InvocationsGetResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Invocations/Get";
    const WATCH: bool = false;
}
impl OperationRequest for InvocationsCancelRequest {
    type Response = InvocationsCancelResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Invocations/Cancel";
    const WATCH: bool = false;
}
impl OperationRequest for InvocationsWatchRequest {
    type Response = InvocationsWatchResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Invocations/Watch";
    const WATCH: bool = true;
}
impl OperationRequest for OperationsListRequest {
    type Response = OperationsListResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Operations/List";
    const WATCH: bool = false;
}
impl OperationRequest for OperationsGetRequest {
    type Response = OperationsGetResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Operations/Get";
    const WATCH: bool = false;
}
impl OperationRequest for SettingsListRequest {
    type Response = SettingsListResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Settings/List";
    const WATCH: bool = false;
}
impl OperationRequest for TimeGetRequest {
    type Response = TimeGetResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Time/Get";
    const WATCH: bool = false;
}
impl OperationRequest for TimeSleepRequest {
    type Response = TimeSleepResponse;
    const ID: &'static str = "harmos.plugin.host.v1.Time/Sleep";
    const WATCH: bool = false;
}
