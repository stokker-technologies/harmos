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
impl Message for harmos_protocol::context::StorageAdvanceProjectionRequest {
    fn name() -> String { "harmos.context.v1.StorageAdvanceProjectionRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageAdvanceProjectionRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageAdvanceProjectionResponse {
    fn name() -> String { "harmos.context.v1.StorageAdvanceProjectionResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageAdvanceProjectionResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageAppendFileBytesRequest {
    fn name() -> String { "harmos.context.v1.StorageAppendFileBytesRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageAppendFileBytesRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageAppendFileBytesResponse {
    fn name() -> String { "harmos.context.v1.StorageAppendFileBytesResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageAppendFileBytesResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageAppendObjectRevisionsRequest {
    fn name() -> String { "harmos.context.v1.StorageAppendObjectRevisionsRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageAppendObjectRevisionsRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageAppendObjectRevisionsResponse {
    fn name() -> String { "harmos.context.v1.StorageAppendObjectRevisionsResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageAppendObjectRevisionsResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageAppendRecordsRequest {
    fn name() -> String { "harmos.context.v1.StorageAppendRecordsRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageAppendRecordsRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageAppendRecordsResponse {
    fn name() -> String { "harmos.context.v1.StorageAppendRecordsResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageAppendRecordsResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageAppendSamplesRequest {
    fn name() -> String { "harmos.context.v1.StorageAppendSamplesRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageAppendSamplesRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageAppendSamplesResponse {
    fn name() -> String { "harmos.context.v1.StorageAppendSamplesResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageAppendSamplesResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageCapabilitiesRequest {
    fn name() -> String { "harmos.context.v1.StorageCapabilitiesRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageCapabilitiesRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageCapabilitiesResponse {
    fn name() -> String { "harmos.context.v1.StorageCapabilitiesResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageCapabilitiesResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageCaptureFileRequest {
    fn name() -> String { "harmos.context.v1.StorageCaptureFileRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageCaptureFileRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageCaptureFileResponse {
    fn name() -> String { "harmos.context.v1.StorageCaptureFileResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageCaptureFileResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageCreateFileProductionRequest {
    fn name() -> String { "harmos.context.v1.StorageCreateFileProductionRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageCreateFileProductionRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageCreateFileProductionResponse {
    fn name() -> String { "harmos.context.v1.StorageCreateFileProductionResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageCreateFileProductionResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageCreateFileWriterRequest {
    fn name() -> String { "harmos.context.v1.StorageCreateFileWriterRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageCreateFileWriterRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageCreateFileWriterResponse {
    fn name() -> String { "harmos.context.v1.StorageCreateFileWriterResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageCreateFileWriterResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageCreateSnapshotRequest {
    fn name() -> String { "harmos.context.v1.StorageCreateSnapshotRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageCreateSnapshotRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageCreateSnapshotResponse {
    fn name() -> String { "harmos.context.v1.StorageCreateSnapshotResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageCreateSnapshotResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageDeleteFileProductionRequest {
    fn name() -> String { "harmos.context.v1.StorageDeleteFileProductionRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageDeleteFileProductionRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageDeleteFileProductionResponse {
    fn name() -> String { "harmos.context.v1.StorageDeleteFileProductionResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageDeleteFileProductionResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageDeleteSnapshotRequest {
    fn name() -> String { "harmos.context.v1.StorageDeleteSnapshotRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageDeleteSnapshotRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageDeleteSnapshotResponse {
    fn name() -> String { "harmos.context.v1.StorageDeleteSnapshotResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageDeleteSnapshotResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetFileBytesRequest {
    fn name() -> String { "harmos.context.v1.StorageGetFileBytesRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetFileBytesRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetFileBytesResponse {
    fn name() -> String { "harmos.context.v1.StorageGetFileBytesResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetFileBytesResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetFileHashRequest {
    fn name() -> String { "harmos.context.v1.StorageGetFileHashRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetFileHashRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetFileHashResponse {
    fn name() -> String { "harmos.context.v1.StorageGetFileHashResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetFileHashResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetFileProductionRequest {
    fn name() -> String { "harmos.context.v1.StorageGetFileProductionRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetFileProductionRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetFileProductionResponse {
    fn name() -> String { "harmos.context.v1.StorageGetFileProductionResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetFileProductionResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetIndexDefinitionRequest {
    fn name() -> String { "harmos.context.v1.StorageGetIndexDefinitionRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetIndexDefinitionRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetIndexDefinitionResponse {
    fn name() -> String { "harmos.context.v1.StorageGetIndexDefinitionResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetIndexDefinitionResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetObjectRequest {
    fn name() -> String { "harmos.context.v1.StorageGetObjectRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetObjectRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetObjectResponse {
    fn name() -> String { "harmos.context.v1.StorageGetObjectResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetObjectResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetProjectionDefinitionRequest {
    fn name() -> String { "harmos.context.v1.StorageGetProjectionDefinitionRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetProjectionDefinitionRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetProjectionDefinitionResponse {
    fn name() -> String { "harmos.context.v1.StorageGetProjectionDefinitionResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetProjectionDefinitionResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetProjectionStatusRequest {
    fn name() -> String { "harmos.context.v1.StorageGetProjectionStatusRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetProjectionStatusRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageGetProjectionStatusResponse {
    fn name() -> String { "harmos.context.v1.StorageGetProjectionStatusResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageGetProjectionStatusResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageListColumnRowsRequest {
    fn name() -> String { "harmos.context.v1.StorageListColumnRowsRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageListColumnRowsRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageListColumnRowsResponse {
    fn name() -> String { "harmos.context.v1.StorageListColumnRowsResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageListColumnRowsResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageListDirectoryEntriesRequest {
    fn name() -> String { "harmos.context.v1.StorageListDirectoryEntriesRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageListDirectoryEntriesRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageListDirectoryEntriesResponse {
    fn name() -> String { "harmos.context.v1.StorageListDirectoryEntriesResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageListDirectoryEntriesResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageListIndexEntriesRequest {
    fn name() -> String { "harmos.context.v1.StorageListIndexEntriesRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageListIndexEntriesRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageListIndexEntriesResponse {
    fn name() -> String { "harmos.context.v1.StorageListIndexEntriesResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageListIndexEntriesResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageListProjectionRowsRequest {
    fn name() -> String { "harmos.context.v1.StorageListProjectionRowsRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageListProjectionRowsRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageListProjectionRowsResponse {
    fn name() -> String { "harmos.context.v1.StorageListProjectionRowsResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageListProjectionRowsResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageListRowsRequest {
    fn name() -> String { "harmos.context.v1.StorageListRowsRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageListRowsRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageListRowsResponse {
    fn name() -> String { "harmos.context.v1.StorageListRowsResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageListRowsResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StoragePublishFileProductionRequest {
    fn name() -> String { "harmos.context.v1.StoragePublishFileProductionRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StoragePublishFileProductionRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StoragePublishFileProductionResponse {
    fn name() -> String { "harmos.context.v1.StoragePublishFileProductionResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StoragePublishFileProductionResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageSetIndexDefinitionRequest {
    fn name() -> String { "harmos.context.v1.StorageSetIndexDefinitionRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageSetIndexDefinitionRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageSetIndexDefinitionResponse {
    fn name() -> String { "harmos.context.v1.StorageSetIndexDefinitionResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageSetIndexDefinitionResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageSetObjectRequest {
    fn name() -> String { "harmos.context.v1.StorageSetObjectRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageSetObjectRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageSetObjectResponse {
    fn name() -> String { "harmos.context.v1.StorageSetObjectResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageSetObjectResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageSetProjectionDefinitionRequest {
    fn name() -> String { "harmos.context.v1.StorageSetProjectionDefinitionRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageSetProjectionDefinitionRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageSetProjectionDefinitionResponse {
    fn name() -> String { "harmos.context.v1.StorageSetProjectionDefinitionResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageSetProjectionDefinitionResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageWatchDirectoryEntriesRequest {
    fn name() -> String { "harmos.context.v1.StorageWatchDirectoryEntriesRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageWatchDirectoryEntriesRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageWatchDirectoryEntriesResponse {
    fn name() -> String { "harmos.context.v1.StorageWatchDirectoryEntriesResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageWatchDirectoryEntriesResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageWatchFileBytesRequest {
    fn name() -> String { "harmos.context.v1.StorageWatchFileBytesRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageWatchFileBytesRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageWatchFileBytesResponse {
    fn name() -> String { "harmos.context.v1.StorageWatchFileBytesResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageWatchFileBytesResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageWatchFileProductionRequest {
    fn name() -> String { "harmos.context.v1.StorageWatchFileProductionRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageWatchFileProductionRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageWatchFileProductionResponse {
    fn name() -> String { "harmos.context.v1.StorageWatchFileProductionResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageWatchFileProductionResponse.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageWatchRowsRequest {
    fn name() -> String { "harmos.context.v1.StorageWatchRowsRequest".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageWatchRowsRequest.bin")[..]).expect("generated context wrapper")
    }
}
impl Message for harmos_protocol::context::StorageWatchRowsResponse {
    fn name() -> String { "harmos.context.v1.StorageWatchRowsResponse".into() }
    fn types() -> prost_types::FileDescriptorSet {
        prost::Message::decode(&include_bytes!("types/harmos.context.v1.StorageWatchRowsResponse.bin")[..]).expect("generated context wrapper")
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
impl OperationRequest for harmos_protocol::context::StorageCreateFileProductionRequest {
    type Response = harmos_protocol::context::StorageCreateFileProductionResponse;
    const ID: &'static str = "harmos.context.v1.Storage/CreateFileProduction";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageGetFileProductionRequest {
    type Response = harmos_protocol::context::StorageGetFileProductionResponse;
    const ID: &'static str = "harmos.context.v1.Storage/GetFileProduction";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageAppendFileBytesRequest {
    type Response = harmos_protocol::context::StorageAppendFileBytesResponse;
    const ID: &'static str = "harmos.context.v1.Storage/AppendFileBytes";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageCreateFileWriterRequest {
    type Response = harmos_protocol::context::StorageCreateFileWriterResponse;
    const ID: &'static str = "harmos.context.v1.Storage/CreateFileWriter";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StoragePublishFileProductionRequest {
    type Response = harmos_protocol::context::StoragePublishFileProductionResponse;
    const ID: &'static str = "harmos.context.v1.Storage/PublishFileProduction";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageDeleteFileProductionRequest {
    type Response = harmos_protocol::context::StorageDeleteFileProductionResponse;
    const ID: &'static str = "harmos.context.v1.Storage/DeleteFileProduction";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageGetFileBytesRequest {
    type Response = harmos_protocol::context::StorageGetFileBytesResponse;
    const ID: &'static str = "harmos.context.v1.Storage/GetFileBytes";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageCapabilitiesRequest {
    type Response = harmos_protocol::context::StorageCapabilitiesResponse;
    const ID: &'static str = "harmos.context.v1.Storage/Capabilities";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageCreateSnapshotRequest {
    type Response = harmos_protocol::context::StorageCreateSnapshotResponse;
    const ID: &'static str = "harmos.context.v1.Storage/CreateSnapshot";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageDeleteSnapshotRequest {
    type Response = harmos_protocol::context::StorageDeleteSnapshotResponse;
    const ID: &'static str = "harmos.context.v1.Storage/DeleteSnapshot";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageListColumnRowsRequest {
    type Response = harmos_protocol::context::StorageListColumnRowsResponse;
    const ID: &'static str = "harmos.context.v1.Storage/ListColumnRows";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageListIndexEntriesRequest {
    type Response = harmos_protocol::context::StorageListIndexEntriesResponse;
    const ID: &'static str = "harmos.context.v1.Storage/ListIndexEntries";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageSetIndexDefinitionRequest {
    type Response = harmos_protocol::context::StorageSetIndexDefinitionResponse;
    const ID: &'static str = "harmos.context.v1.Storage/SetIndexDefinition";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageSetProjectionDefinitionRequest {
    type Response = harmos_protocol::context::StorageSetProjectionDefinitionResponse;
    const ID: &'static str = "harmos.context.v1.Storage/SetProjectionDefinition";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageAdvanceProjectionRequest {
    type Response = harmos_protocol::context::StorageAdvanceProjectionResponse;
    const ID: &'static str = "harmos.context.v1.Storage/AdvanceProjection";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageListProjectionRowsRequest {
    type Response = harmos_protocol::context::StorageListProjectionRowsResponse;
    const ID: &'static str = "harmos.context.v1.Storage/ListProjectionRows";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageGetProjectionStatusRequest {
    type Response = harmos_protocol::context::StorageGetProjectionStatusResponse;
    const ID: &'static str = "harmos.context.v1.Storage/GetProjectionStatus";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageSetObjectRequest {
    type Response = harmos_protocol::context::StorageSetObjectResponse;
    const ID: &'static str = "harmos.context.v1.Storage/SetObject";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageListRowsRequest {
    type Response = harmos_protocol::context::StorageListRowsResponse;
    const ID: &'static str = "harmos.context.v1.Storage/ListRows";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageWatchRowsRequest {
    type Response = harmos_protocol::context::StorageWatchRowsResponse;
    const ID: &'static str = "harmos.context.v1.Storage/WatchRows";
    const WATCH: bool = true;
}
impl OperationRequest for harmos_protocol::context::StorageAppendRecordsRequest {
    type Response = harmos_protocol::context::StorageAppendRecordsResponse;
    const ID: &'static str = "harmos.context.v1.Storage/AppendRecords";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageAppendSamplesRequest {
    type Response = harmos_protocol::context::StorageAppendSamplesResponse;
    const ID: &'static str = "harmos.context.v1.Storage/AppendSamples";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageAppendObjectRevisionsRequest {
    type Response = harmos_protocol::context::StorageAppendObjectRevisionsResponse;
    const ID: &'static str = "harmos.context.v1.Storage/AppendObjectRevisions";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageGetFileHashRequest {
    type Response = harmos_protocol::context::StorageGetFileHashResponse;
    const ID: &'static str = "harmos.context.v1.Storage/GetFileHash";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageCaptureFileRequest {
    type Response = harmos_protocol::context::StorageCaptureFileResponse;
    const ID: &'static str = "harmos.context.v1.Storage/CaptureFile";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageWatchFileBytesRequest {
    type Response = harmos_protocol::context::StorageWatchFileBytesResponse;
    const ID: &'static str = "harmos.context.v1.Storage/WatchFileBytes";
    const WATCH: bool = true;
}
impl OperationRequest for harmos_protocol::context::StorageWatchFileProductionRequest {
    type Response = harmos_protocol::context::StorageWatchFileProductionResponse;
    const ID: &'static str = "harmos.context.v1.Storage/WatchFileProduction";
    const WATCH: bool = true;
}
impl OperationRequest for harmos_protocol::context::StorageListDirectoryEntriesRequest {
    type Response = harmos_protocol::context::StorageListDirectoryEntriesResponse;
    const ID: &'static str = "harmos.context.v1.Storage/ListDirectoryEntries";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageWatchDirectoryEntriesRequest {
    type Response = harmos_protocol::context::StorageWatchDirectoryEntriesResponse;
    const ID: &'static str = "harmos.context.v1.Storage/WatchDirectoryEntries";
    const WATCH: bool = true;
}
impl OperationRequest for harmos_protocol::context::StorageGetIndexDefinitionRequest {
    type Response = harmos_protocol::context::StorageGetIndexDefinitionResponse;
    const ID: &'static str = "harmos.context.v1.Storage/GetIndexDefinition";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageGetProjectionDefinitionRequest {
    type Response = harmos_protocol::context::StorageGetProjectionDefinitionResponse;
    const ID: &'static str = "harmos.context.v1.Storage/GetProjectionDefinition";
    const WATCH: bool = false;
}
impl OperationRequest for harmos_protocol::context::StorageGetObjectRequest {
    type Response = harmos_protocol::context::StorageGetObjectResponse;
    const ID: &'static str = "harmos.context.v1.Storage/GetObject";
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
