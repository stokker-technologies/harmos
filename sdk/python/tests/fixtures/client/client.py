# @generated from protocol descriptors; do not edit.
"""Typed Client Resources."""

import grpc

import harmos.v1.access.certificates_pb2 as messages_0
import harmos.v1.access.listener_pb2 as messages_1
import harmos.v1.access.pairings_pb2 as messages_2
import harmos.v1.catalog_entries_pb2 as messages_3
import harmos.v1.invocations_pb2 as messages_4
import harmos.v1.plugins_pb2 as messages_5
import harmos.v1.status.diagnostics_pb2 as messages_6
import harmos.v1.status.logs_pb2 as messages_7
import harmos.v1.status.trace_events_pb2 as messages_8
import harmos.v1.status_pb2 as messages_9
import harmos.v1.storage_pb2 as messages_10
import harmos.v1.time_pb2 as messages_11
import tests.fixtures.client.application.v1.jobs.progress_pb2 as messages_12
import tests.fixtures.client.application.v1.jobs_pb2 as messages_13
from harmos.client_transport import Metadata, stream, unary


class AccessCertificatesResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def create(
        self,
        request: messages_0.CertificatesCreateRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_0.CertificatesCreateRequest, messages_0.CertificatesCreateResponse]:
        return unary(
            self._channel,
            "/harmos.v1.access.Certificates/Create",
            request,
            messages_0.CertificatesCreateResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class AccessListenerResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def get(
        self,
        request: messages_1.ListenerGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_1.ListenerGetRequest, messages_1.ListenerGetResponse]:
        return unary(
            self._channel,
            "/harmos.v1.access.Listener/Get",
            request,
            messages_1.ListenerGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def set(
        self,
        request: messages_1.ListenerSetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_1.ListenerSetRequest, messages_1.ListenerSetResponse]:
        return unary(
            self._channel,
            "/harmos.v1.access.Listener/Set",
            request,
            messages_1.ListenerSetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class AccessPairingsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def create(
        self,
        request: messages_2.PairingsCreateRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_2.PairingsCreateRequest, messages_2.PairingsCreateResponse]:
        return unary(
            self._channel,
            "/harmos.v1.access.Pairings/Create",
            request,
            messages_2.PairingsCreateResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def list(
        self,
        request: messages_2.PairingsListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_2.PairingsListRequest, messages_2.PairingsListResponse]:
        return unary(
            self._channel,
            "/harmos.v1.access.Pairings/List",
            request,
            messages_2.PairingsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def delete(
        self,
        request: messages_2.PairingsDeleteRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_2.PairingsDeleteRequest, messages_2.PairingsDeleteResponse]:
        return unary(
            self._channel,
            "/harmos.v1.access.Pairings/Delete",
            request,
            messages_2.PairingsDeleteResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class JobsProgressResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def watch(
        self,
        request: messages_12.ProgressWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_12.ProgressWatchRequest, messages_12.ProgressWatchResponse]:
        return stream(
            self._channel,
            "/application.v1.jobs.Progress/Watch",
            request,
            messages_12.ProgressWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class StatusDiagnosticsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def get(
        self,
        request: messages_6.DiagnosticsGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_6.DiagnosticsGetRequest, messages_6.DiagnosticsGetResponse]:
        return unary(
            self._channel,
            "/harmos.v1.status.Diagnostics/Get",
            request,
            messages_6.DiagnosticsGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class StatusLogsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def list(
        self,
        request: messages_7.LogsListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_7.LogsListRequest, messages_7.LogsListResponse]:
        return unary(
            self._channel,
            "/harmos.v1.status.Logs/List",
            request,
            messages_7.LogsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch(
        self,
        request: messages_7.LogsWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_7.LogsWatchRequest, messages_7.LogsWatchResponse]:
        return stream(
            self._channel,
            "/harmos.v1.status.Logs/Watch",
            request,
            messages_7.LogsWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class StatusTraceEventsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def list(
        self,
        request: messages_8.TraceEventsListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_8.TraceEventsListRequest, messages_8.TraceEventsListResponse]:
        return unary(
            self._channel,
            "/harmos.v1.status.TraceEvents/List",
            request,
            messages_8.TraceEventsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch(
        self,
        request: messages_8.TraceEventsWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_8.TraceEventsWatchRequest, messages_8.TraceEventsWatchResponse]:
        return stream(
            self._channel,
            "/harmos.v1.status.TraceEvents/Watch",
            request,
            messages_8.TraceEventsWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class AccessResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel
        self.certificates: AccessCertificatesResource = AccessCertificatesResource(channel)
        self.listener: AccessListenerResource = AccessListenerResource(channel)
        self.pairings: AccessPairingsResource = AccessPairingsResource(channel)


class CatalogEntriesResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def list(
        self,
        request: messages_3.CatalogEntriesListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_3.CatalogEntriesListRequest, messages_3.CatalogEntriesListResponse]:
        return unary(
            self._channel,
            "/harmos.v1.CatalogEntries/List",
            request,
            messages_3.CatalogEntriesListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class InvocationsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def invoke(
        self,
        request: messages_4.InvocationsInvokeRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_4.InvocationsInvokeRequest, messages_4.InvocationsInvokeResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Invocations/Invoke",
            request,
            messages_4.InvocationsInvokeResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def get(
        self,
        request: messages_4.InvocationsGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_4.InvocationsGetRequest, messages_4.InvocationsGetResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Invocations/Get",
            request,
            messages_4.InvocationsGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def cancel(
        self,
        request: messages_4.InvocationsCancelRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_4.InvocationsCancelRequest, messages_4.InvocationsCancelResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Invocations/Cancel",
            request,
            messages_4.InvocationsCancelResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch(
        self,
        request: messages_4.InvocationsWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_4.InvocationsWatchRequest, messages_4.InvocationsWatchResponse]:
        return stream(
            self._channel,
            "/harmos.v1.Invocations/Watch",
            request,
            messages_4.InvocationsWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class JobsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel
        self.progress: JobsProgressResource = JobsProgressResource(channel)

    def start(
        self,
        request: messages_13.JobsStartRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_13.JobsStartRequest, messages_13.JobsStartResponse]:
        return unary(
            self._channel,
            "/application.v1.Jobs/Start",
            request,
            messages_13.JobsStartResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class PluginsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def get(
        self,
        request: messages_5.PluginsGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_5.PluginsGetRequest, messages_5.PluginsGetResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Plugins/Get",
            request,
            messages_5.PluginsGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def list(
        self,
        request: messages_5.PluginsListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_5.PluginsListRequest, messages_5.PluginsListResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Plugins/List",
            request,
            messages_5.PluginsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch(
        self,
        request: messages_5.PluginsWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_5.PluginsWatchRequest, messages_5.PluginsWatchResponse]:
        return stream(
            self._channel,
            "/harmos.v1.Plugins/Watch",
            request,
            messages_5.PluginsWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def create(
        self,
        request: messages_5.PluginsCreateRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_5.PluginsCreateRequest, messages_5.PluginsCreateResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Plugins/Create",
            request,
            messages_5.PluginsCreateResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def set(
        self,
        request: messages_5.PluginsSetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_5.PluginsSetRequest, messages_5.PluginsSetResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Plugins/Set",
            request,
            messages_5.PluginsSetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def delete(
        self,
        request: messages_5.PluginsDeleteRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_5.PluginsDeleteRequest, messages_5.PluginsDeleteResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Plugins/Delete",
            request,
            messages_5.PluginsDeleteResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def restart(
        self,
        request: messages_5.PluginsRestartRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_5.PluginsRestartRequest, messages_5.PluginsRestartResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Plugins/Restart",
            request,
            messages_5.PluginsRestartResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class StatusResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel
        self.diagnostics: StatusDiagnosticsResource = StatusDiagnosticsResource(channel)
        self.logs: StatusLogsResource = StatusLogsResource(channel)
        self.trace_events: StatusTraceEventsResource = StatusTraceEventsResource(channel)

    def get(
        self,
        request: messages_9.StatusGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_9.StatusGetRequest, messages_9.StatusGetResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Status/Get",
            request,
            messages_9.StatusGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch(
        self,
        request: messages_9.StatusWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_9.StatusWatchRequest, messages_9.StatusWatchResponse]:
        return stream(
            self._channel,
            "/harmos.v1.Status/Watch",
            request,
            messages_9.StatusWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class StorageResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def create_file_production(
        self,
        request: messages_10.StorageCreateFileProductionRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageCreateFileProductionRequest, messages_10.StorageCreateFileProductionResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/CreateFileProduction",
            request,
            messages_10.StorageCreateFileProductionResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def get_file_production(
        self,
        request: messages_10.StorageGetFileProductionRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageGetFileProductionRequest, messages_10.StorageGetFileProductionResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/GetFileProduction",
            request,
            messages_10.StorageGetFileProductionResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def append_file_bytes(
        self,
        request: messages_10.StorageAppendFileBytesRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageAppendFileBytesRequest, messages_10.StorageAppendFileBytesResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/AppendFileBytes",
            request,
            messages_10.StorageAppendFileBytesResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def create_file_writer(
        self,
        request: messages_10.StorageCreateFileWriterRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageCreateFileWriterRequest, messages_10.StorageCreateFileWriterResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/CreateFileWriter",
            request,
            messages_10.StorageCreateFileWriterResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def publish_file_production(
        self,
        request: messages_10.StoragePublishFileProductionRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StoragePublishFileProductionRequest, messages_10.StoragePublishFileProductionResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/PublishFileProduction",
            request,
            messages_10.StoragePublishFileProductionResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def delete_file_production(
        self,
        request: messages_10.StorageDeleteFileProductionRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageDeleteFileProductionRequest, messages_10.StorageDeleteFileProductionResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/DeleteFileProduction",
            request,
            messages_10.StorageDeleteFileProductionResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def get_file_bytes(
        self,
        request: messages_10.StorageGetFileBytesRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageGetFileBytesRequest, messages_10.StorageGetFileBytesResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/GetFileBytes",
            request,
            messages_10.StorageGetFileBytesResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def capabilities(
        self,
        request: messages_10.StorageCapabilitiesRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageCapabilitiesRequest, messages_10.StorageCapabilitiesResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/Capabilities",
            request,
            messages_10.StorageCapabilitiesResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def create_snapshot(
        self,
        request: messages_10.StorageCreateSnapshotRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageCreateSnapshotRequest, messages_10.StorageCreateSnapshotResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/CreateSnapshot",
            request,
            messages_10.StorageCreateSnapshotResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def delete_snapshot(
        self,
        request: messages_10.StorageDeleteSnapshotRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageDeleteSnapshotRequest, messages_10.StorageDeleteSnapshotResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/DeleteSnapshot",
            request,
            messages_10.StorageDeleteSnapshotResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def list_column_rows(
        self,
        request: messages_10.StorageListColumnRowsRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageListColumnRowsRequest, messages_10.StorageListColumnRowsResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/ListColumnRows",
            request,
            messages_10.StorageListColumnRowsResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def list_index_entries(
        self,
        request: messages_10.StorageListIndexEntriesRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageListIndexEntriesRequest, messages_10.StorageListIndexEntriesResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/ListIndexEntries",
            request,
            messages_10.StorageListIndexEntriesResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def set_index_definition(
        self,
        request: messages_10.StorageSetIndexDefinitionRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageSetIndexDefinitionRequest, messages_10.StorageSetIndexDefinitionResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/SetIndexDefinition",
            request,
            messages_10.StorageSetIndexDefinitionResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def set_projection_definition(
        self,
        request: messages_10.StorageSetProjectionDefinitionRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageSetProjectionDefinitionRequest, messages_10.StorageSetProjectionDefinitionResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/SetProjectionDefinition",
            request,
            messages_10.StorageSetProjectionDefinitionResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def advance_projection(
        self,
        request: messages_10.StorageAdvanceProjectionRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageAdvanceProjectionRequest, messages_10.StorageAdvanceProjectionResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/AdvanceProjection",
            request,
            messages_10.StorageAdvanceProjectionResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def list_projection_rows(
        self,
        request: messages_10.StorageListProjectionRowsRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageListProjectionRowsRequest, messages_10.StorageListProjectionRowsResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/ListProjectionRows",
            request,
            messages_10.StorageListProjectionRowsResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def get_projection_status(
        self,
        request: messages_10.StorageGetProjectionStatusRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageGetProjectionStatusRequest, messages_10.StorageGetProjectionStatusResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/GetProjectionStatus",
            request,
            messages_10.StorageGetProjectionStatusResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def set_object(
        self,
        request: messages_10.StorageSetObjectRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageSetObjectRequest, messages_10.StorageSetObjectResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/SetObject",
            request,
            messages_10.StorageSetObjectResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def list_rows(
        self,
        request: messages_10.StorageListRowsRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageListRowsRequest, messages_10.StorageListRowsResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/ListRows",
            request,
            messages_10.StorageListRowsResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch_rows(
        self,
        request: messages_10.StorageWatchRowsRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_10.StorageWatchRowsRequest, messages_10.StorageWatchRowsResponse]:
        return stream(
            self._channel,
            "/harmos.v1.Storage/WatchRows",
            request,
            messages_10.StorageWatchRowsResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def append_records(
        self,
        request: messages_10.StorageAppendRecordsRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageAppendRecordsRequest, messages_10.StorageAppendRecordsResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/AppendRecords",
            request,
            messages_10.StorageAppendRecordsResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def append_samples(
        self,
        request: messages_10.StorageAppendSamplesRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageAppendSamplesRequest, messages_10.StorageAppendSamplesResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/AppendSamples",
            request,
            messages_10.StorageAppendSamplesResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def append_object_revisions(
        self,
        request: messages_10.StorageAppendObjectRevisionsRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageAppendObjectRevisionsRequest, messages_10.StorageAppendObjectRevisionsResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/AppendObjectRevisions",
            request,
            messages_10.StorageAppendObjectRevisionsResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def get_file_hash(
        self,
        request: messages_10.StorageGetFileHashRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageGetFileHashRequest, messages_10.StorageGetFileHashResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/GetFileHash",
            request,
            messages_10.StorageGetFileHashResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def capture_file(
        self,
        request: messages_10.StorageCaptureFileRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageCaptureFileRequest, messages_10.StorageCaptureFileResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/CaptureFile",
            request,
            messages_10.StorageCaptureFileResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch_file_bytes(
        self,
        request: messages_10.StorageWatchFileBytesRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_10.StorageWatchFileBytesRequest, messages_10.StorageWatchFileBytesResponse]:
        return stream(
            self._channel,
            "/harmos.v1.Storage/WatchFileBytes",
            request,
            messages_10.StorageWatchFileBytesResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch_file_production(
        self,
        request: messages_10.StorageWatchFileProductionRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_10.StorageWatchFileProductionRequest, messages_10.StorageWatchFileProductionResponse]:
        return stream(
            self._channel,
            "/harmos.v1.Storage/WatchFileProduction",
            request,
            messages_10.StorageWatchFileProductionResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def list_directory_entries(
        self,
        request: messages_10.StorageListDirectoryEntriesRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageListDirectoryEntriesRequest, messages_10.StorageListDirectoryEntriesResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/ListDirectoryEntries",
            request,
            messages_10.StorageListDirectoryEntriesResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch_directory_entries(
        self,
        request: messages_10.StorageWatchDirectoryEntriesRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_10.StorageWatchDirectoryEntriesRequest, messages_10.StorageWatchDirectoryEntriesResponse]:
        return stream(
            self._channel,
            "/harmos.v1.Storage/WatchDirectoryEntries",
            request,
            messages_10.StorageWatchDirectoryEntriesResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def get_index_definition(
        self,
        request: messages_10.StorageGetIndexDefinitionRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageGetIndexDefinitionRequest, messages_10.StorageGetIndexDefinitionResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/GetIndexDefinition",
            request,
            messages_10.StorageGetIndexDefinitionResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def get_projection_definition(
        self,
        request: messages_10.StorageGetProjectionDefinitionRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageGetProjectionDefinitionRequest, messages_10.StorageGetProjectionDefinitionResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/GetProjectionDefinition",
            request,
            messages_10.StorageGetProjectionDefinitionResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def get_object(
        self,
        request: messages_10.StorageGetObjectRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.StorageGetObjectRequest, messages_10.StorageGetObjectResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Storage/GetObject",
            request,
            messages_10.StorageGetObjectResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class TimeResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def get(
        self,
        request: messages_11.TimeGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_11.TimeGetRequest, messages_11.TimeGetResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Time/Get",
            request,
            messages_11.TimeGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def sleep(
        self,
        request: messages_11.TimeSleepRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_11.TimeSleepRequest, messages_11.TimeSleepResponse]:
        return unary(
            self._channel,
            "/harmos.v1.Time/Sleep",
            request,
            messages_11.TimeSleepResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class ClientRoot:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel
        self.access: AccessResource = AccessResource(channel)
        self.catalog_entries: CatalogEntriesResource = CatalogEntriesResource(channel)
        self.invocations: InvocationsResource = InvocationsResource(channel)
        self.jobs: JobsResource = JobsResource(channel)
        self.plugins: PluginsResource = PluginsResource(channel)
        self.status: StatusResource = StatusResource(channel)
        self.storage: StorageResource = StorageResource(channel)
        self.time: TimeResource = TimeResource(channel)
