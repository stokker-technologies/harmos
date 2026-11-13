# @generated from protocol descriptors; do not edit.
"""Typed Client Resources."""

import grpc

import harmos._wire.client.v1.access.certificates_pb2 as messages_0
import harmos._wire.client.v1.access.listener_pb2 as messages_1
import harmos._wire.client.v1.access.pairings_pb2 as messages_2
import harmos._wire.client.v1.invocations_pb2 as messages_3
import harmos._wire.client.v1.operations_pb2 as messages_4
import harmos._wire.client.v1.plugins.settings_pb2 as messages_5
import harmos._wire.client.v1.plugins_pb2 as messages_6
import harmos._wire.client.v1.schemas_pb2 as messages_7
import harmos._wire.client.v1.status.diagnostics_pb2 as messages_8
import harmos._wire.client.v1.status.logs_pb2 as messages_9
import harmos._wire.client.v1.status.trace_events_pb2 as messages_10
import harmos._wire.client.v1.status_pb2 as messages_11
import harmos._wire.client.v1.streams_pb2 as messages_12
import harmos._wire.client.v1.time_pb2 as messages_13
import tests.fixtures.client.application.v1.jobs.progress_pb2 as messages_14
import tests.fixtures.client.application.v1.jobs_pb2 as messages_15
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
            "/harmos.client.v1.access.Certificates/Create",
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
            "/harmos.client.v1.access.Listener/Get",
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
            "/harmos.client.v1.access.Listener/Set",
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
            "/harmos.client.v1.access.Pairings/Create",
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
            "/harmos.client.v1.access.Pairings/List",
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
            "/harmos.client.v1.access.Pairings/Delete",
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
        request: messages_14.ProgressWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_14.ProgressWatchRequest, messages_14.ProgressWatchResponse]:
        return stream(
            self._channel,
            "/application.v1.jobs.Progress/Watch",
            request,
            messages_14.ProgressWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class PluginsSettingsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def list(
        self,
        request: messages_5.SettingsListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_5.SettingsListRequest, messages_5.SettingsListResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.plugins.Settings/List",
            request,
            messages_5.SettingsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def set(
        self,
        request: messages_5.SettingsSetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_5.SettingsSetRequest, messages_5.SettingsSetResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.plugins.Settings/Set",
            request,
            messages_5.SettingsSetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class StatusDiagnosticsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def get(
        self,
        request: messages_8.DiagnosticsGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_8.DiagnosticsGetRequest, messages_8.DiagnosticsGetResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.status.Diagnostics/Get",
            request,
            messages_8.DiagnosticsGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class StatusLogsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def list(
        self,
        request: messages_9.LogsListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_9.LogsListRequest, messages_9.LogsListResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.status.Logs/List",
            request,
            messages_9.LogsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch(
        self,
        request: messages_9.LogsWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_9.LogsWatchRequest, messages_9.LogsWatchResponse]:
        return stream(
            self._channel,
            "/harmos.client.v1.status.Logs/Watch",
            request,
            messages_9.LogsWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class StatusTraceEventsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def list(
        self,
        request: messages_10.TraceEventsListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_10.TraceEventsListRequest, messages_10.TraceEventsListResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.status.TraceEvents/List",
            request,
            messages_10.TraceEventsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch(
        self,
        request: messages_10.TraceEventsWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_10.TraceEventsWatchRequest, messages_10.TraceEventsWatchResponse]:
        return stream(
            self._channel,
            "/harmos.client.v1.status.TraceEvents/Watch",
            request,
            messages_10.TraceEventsWatchResponse,
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


class InvocationsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def invoke(
        self,
        request: messages_3.InvocationsInvokeRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_3.InvocationsInvokeRequest, messages_3.InvocationsInvokeResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Invocations/Invoke",
            request,
            messages_3.InvocationsInvokeResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def get(
        self,
        request: messages_3.InvocationsGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_3.InvocationsGetRequest, messages_3.InvocationsGetResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Invocations/Get",
            request,
            messages_3.InvocationsGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def cancel(
        self,
        request: messages_3.InvocationsCancelRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_3.InvocationsCancelRequest, messages_3.InvocationsCancelResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Invocations/Cancel",
            request,
            messages_3.InvocationsCancelResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch(
        self,
        request: messages_3.InvocationsWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_3.InvocationsWatchRequest, messages_3.InvocationsWatchResponse]:
        return stream(
            self._channel,
            "/harmos.client.v1.Invocations/Watch",
            request,
            messages_3.InvocationsWatchResponse,
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
        request: messages_15.JobsStartRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_15.JobsStartRequest, messages_15.JobsStartResponse]:
        return unary(
            self._channel,
            "/application.v1.Jobs/Start",
            request,
            messages_15.JobsStartResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class OperationsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def list(
        self,
        request: messages_4.OperationsListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_4.OperationsListRequest, messages_4.OperationsListResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Operations/List",
            request,
            messages_4.OperationsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def get(
        self,
        request: messages_4.OperationsGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_4.OperationsGetRequest, messages_4.OperationsGetResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Operations/Get",
            request,
            messages_4.OperationsGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class PluginsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel
        self.settings: PluginsSettingsResource = PluginsSettingsResource(channel)

    def get(
        self,
        request: messages_6.PluginsGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_6.PluginsGetRequest, messages_6.PluginsGetResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Plugins/Get",
            request,
            messages_6.PluginsGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def list(
        self,
        request: messages_6.PluginsListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_6.PluginsListRequest, messages_6.PluginsListResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Plugins/List",
            request,
            messages_6.PluginsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch(
        self,
        request: messages_6.PluginsWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_6.PluginsWatchRequest, messages_6.PluginsWatchResponse]:
        return stream(
            self._channel,
            "/harmos.client.v1.Plugins/Watch",
            request,
            messages_6.PluginsWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def create(
        self,
        request: messages_6.PluginsCreateRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_6.PluginsCreateRequest, messages_6.PluginsCreateResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Plugins/Create",
            request,
            messages_6.PluginsCreateResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def set(
        self,
        request: messages_6.PluginsSetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_6.PluginsSetRequest, messages_6.PluginsSetResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Plugins/Set",
            request,
            messages_6.PluginsSetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def delete(
        self,
        request: messages_6.PluginsDeleteRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_6.PluginsDeleteRequest, messages_6.PluginsDeleteResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Plugins/Delete",
            request,
            messages_6.PluginsDeleteResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def restart(
        self,
        request: messages_6.PluginsRestartRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_6.PluginsRestartRequest, messages_6.PluginsRestartResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Plugins/Restart",
            request,
            messages_6.PluginsRestartResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def replace(
        self,
        request: messages_6.PluginsReplaceRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_6.PluginsReplaceRequest, messages_6.PluginsReplaceResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Plugins/Replace",
            request,
            messages_6.PluginsReplaceResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class SchemasResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def list(
        self,
        request: messages_7.SchemasListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_7.SchemasListRequest, messages_7.SchemasListResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Schemas/List",
            request,
            messages_7.SchemasListResponse,
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
        request: messages_11.StatusGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_11.StatusGetRequest, messages_11.StatusGetResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Status/Get",
            request,
            messages_11.StatusGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def watch(
        self,
        request: messages_11.StatusWatchRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryStreamCall[messages_11.StatusWatchRequest, messages_11.StatusWatchResponse]:
        return stream(
            self._channel,
            "/harmos.client.v1.Status/Watch",
            request,
            messages_11.StatusWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class StreamsResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def list(
        self,
        request: messages_12.StreamsListRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_12.StreamsListRequest, messages_12.StreamsListResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Streams/List",
            request,
            messages_12.StreamsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class TimeResource:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel

    def get(
        self,
        request: messages_13.TimeGetRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_13.TimeGetRequest, messages_13.TimeGetResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Time/Get",
            request,
            messages_13.TimeGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )

    def sleep(
        self,
        request: messages_13.TimeSleepRequest,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: grpc.CallCredentials | None = None,
    ) -> grpc.aio.UnaryUnaryCall[messages_13.TimeSleepRequest, messages_13.TimeSleepResponse]:
        return unary(
            self._channel,
            "/harmos.client.v1.Time/Sleep",
            request,
            messages_13.TimeSleepResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )


class ClientRoot:
    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel
        self.access: AccessResource = AccessResource(channel)
        self.invocations: InvocationsResource = InvocationsResource(channel)
        self.jobs: JobsResource = JobsResource(channel)
        self.operations: OperationsResource = OperationsResource(channel)
        self.plugins: PluginsResource = PluginsResource(channel)
        self.schemas: SchemasResource = SchemasResource(channel)
        self.status: StatusResource = StatusResource(channel)
        self.streams: StreamsResource = StreamsResource(channel)
        self.time: TimeResource = TimeResource(channel)
