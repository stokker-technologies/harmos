# @generated from protocol descriptors; do not edit.
"""Typed Client Resources."""

from collections.abc import AsyncGenerator
from uuid import uuid4

from google.protobuf.duration_pb2 import Duration
from google.protobuf.timestamp_pb2 import Timestamp
from grpc import CallCredentials
from grpc.aio import AioRpcError, Channel

from harmos._client_transport import Metadata, call_error, stream, unary
from harmos._wire.host.v1.access.certificates_pb2 import CertificatesCreateRequest, CertificatesCreateResponse
from harmos._wire.host.v1.access.listener_pb2 import (
    ListenerGetRequest,
    ListenerGetResponse,
    ListenerSetRequest,
    ListenerSetResponse,
    ListenerValue,
)
from harmos._wire.host.v1.access.pairings_pb2 import (
    PairingsCreateRequest,
    PairingsCreateResponse,
    PairingsDeleteRequest,
    PairingsDeleteResponse,
    PairingsListRequest,
    PairingsListResponse,
)
from harmos._wire.host.v1.invocations_pb2 import (
    InvocationsCancelRequest,
    InvocationsCancelResponse,
    InvocationsCreateRequest,
    InvocationsCreateResponse,
    InvocationsGetRequest,
    InvocationsGetResponse,
    InvocationsWatchRequest,
    InvocationsWatchResponse,
)
from harmos._wire.host.v1.operations_pb2 import OperationsGetRequest, OperationsGetResponse, OperationsListRequest, OperationsListResponse
from harmos._wire.host.v1.plugins.settings_pb2 import SettingsListRequest, SettingsListResponse, SettingsSetRequest, SettingsSetResponse
from harmos._wire.host.v1.plugins_pb2 import (
    PluginEntry,
    PluginPlacement,
    PluginsCreateRequest,
    PluginsCreateResponse,
    PluginsDeleteRequest,
    PluginsDeleteResponse,
    PluginsGetRequest,
    PluginsGetResponse,
    PluginsListRequest,
    PluginsListResponse,
    PluginsReplaceRequest,
    PluginsReplaceResponse,
    PluginsRestartRequest,
    PluginsRestartResponse,
    PluginsSetRequest,
    PluginsSetResponse,
    PluginsWatchRequest,
    PluginsWatchResponse,
    PluginValue,
)
from harmos._wire.host.v1.schemas_pb2 import SchemaDescription, SchemasListRequest, SchemasListResponse
from harmos._wire.host.v1.status.diagnostics_pb2 import DiagnosticsGetRequest, DiagnosticsGetResponse
from harmos._wire.host.v1.status.logs_pb2 import LogsListRequest, LogsListResponse, LogsWatchRequest, LogsWatchResponse
from harmos._wire.host.v1.status.trace_events_pb2 import (
    TraceEventsListRequest,
    TraceEventsListResponse,
    TraceEventsWatchRequest,
    TraceEventsWatchResponse,
)
from harmos._wire.host.v1.status_pb2 import StatusGetRequest, StatusGetResponse, StatusWatchRequest, StatusWatchResponse
from harmos._wire.host.v1.streams_pb2 import StreamIdentity, StreamsListRequest, StreamsListResponse
from harmos._wire.host.v1.time_pb2 import TimeGetRequest, TimeGetResponse, TimeSleepRequest, TimeSleepResponse
from harmos._wire.model.v1.invocation_pb2 import InstanceState
from harmos._wire.model.v1.operation_pb2 import DiscoveredOperation
from harmos._wire.model.v1.page_pb2 import PageRequest
from harmos._wire.model.v1.position_pb2 import Position, ReadCut
from harmos._wire.model.v1.receipt_pb2 import Receipt
from harmos._wire.model.v1.setting_pb2 import SettingValues


class AccessCertificatesResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def create(
        self, *, metadata: Metadata | None = None, timeout: float | None = None, credentials: CallCredentials | None = None
    ) -> str:
        request = CertificatesCreateRequest()
        response = await unary(
            self._channel,
            "/harmos.host.v1.access.Certificates/Create",
            request,
            CertificatesCreateResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="access.certificates.create",
        )
        return response.certificate_pem


class AccessListenerResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def get(
        self, *, metadata: Metadata | None = None, timeout: float | None = None, credentials: CallCredentials | None = None
    ) -> ListenerGetResponse:
        request = ListenerGetRequest()
        response = await unary(
            self._channel,
            "/harmos.host.v1.access.Listener/Get",
            request,
            ListenerGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="access.listener.get",
        )
        return response

    async def set(
        self,
        value: ListenerValue,
        expected_revision: bytes,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> ListenerSetResponse:
        request = ListenerSetRequest(value=value, expected_revision=expected_revision)
        response = await unary(
            self._channel,
            "/harmos.host.v1.access.Listener/Set",
            request,
            ListenerSetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="access.listener.set",
        )
        return response


class AccessPairingsResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def create(
        self, label: str, *, metadata: Metadata | None = None, timeout: float | None = None, credentials: CallCredentials | None = None
    ) -> PairingsCreateResponse:
        request = PairingsCreateRequest(label=label)
        response = await unary(
            self._channel,
            "/harmos.host.v1.access.Pairings/Create",
            request,
            PairingsCreateResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="access.pairings.create",
        )
        return response

    async def list(
        self,
        *,
        at: Position | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> PairingsListResponse:
        request = PairingsListRequest(at=at)
        response = await unary(
            self._channel,
            "/harmos.host.v1.access.Pairings/List",
            request,
            PairingsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="access.pairings.list",
        )
        return response

    async def delete(
        self, name: str, *, metadata: Metadata | None = None, timeout: float | None = None, credentials: CallCredentials | None = None
    ) -> PairingsDeleteResponse:
        request = PairingsDeleteRequest(name=name)
        response = await unary(
            self._channel,
            "/harmos.host.v1.access.Pairings/Delete",
            request,
            PairingsDeleteResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="access.pairings.delete",
        )
        return response


class PluginsSettingsResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def list(
        self,
        name: str,
        *,
        at: ReadCut | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> SettingsListResponse:
        request = SettingsListRequest(name=name, at=at)
        response = await unary(
            self._channel,
            "/harmos.host.v1.plugins.Settings/List",
            request,
            SettingsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="plugins.settings.list",
        )
        return response

    async def set(
        self,
        name: str,
        value: SettingValues,
        expected_revision: bytes,
        *,
        request_id: str | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> SettingsSetResponse:
        request = SettingsSetRequest(
            name=name, value=value, expected_revision=expected_revision, request_id=request_id if request_id is not None else str(uuid4())
        )
        response = await unary(
            self._channel,
            "/harmos.host.v1.plugins.Settings/Set",
            request,
            SettingsSetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="plugins.settings.set",
        )
        return response


class StatusDiagnosticsResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def get(
        self,
        name: str,
        offset: int,
        max_bytes: int,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> DiagnosticsGetResponse:
        request = DiagnosticsGetRequest(name=name, offset=offset, max_bytes=max_bytes)
        response = await unary(
            self._channel,
            "/harmos.host.v1.status.Diagnostics/Get",
            request,
            DiagnosticsGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="status.diagnostics.get",
        )
        return response


class StatusLogsResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def list(
        self,
        *,
        at: Position | None = None,
        page: PageRequest | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> LogsListResponse:
        request = LogsListRequest(at=at, page=page)
        response = await unary(
            self._channel,
            "/harmos.host.v1.status.Logs/List",
            request,
            LogsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="status.logs.list",
        )
        return response

    async def watch(
        self,
        max_bytes: int,
        max_events: int,
        *,
        at: Position | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> AsyncGenerator[LogsWatchResponse]:
        request = LogsWatchRequest(at=at, max_bytes=max_bytes, max_events=max_events)
        call = stream(
            self._channel,
            "/harmos.host.v1.status.Logs/Watch",
            request,
            LogsWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )
        try:
            async for response in call:
                yield response
        except AioRpcError as error:
            raise call_error("status.logs.watch", error) from error
        finally:
            call.cancel()


class StatusTraceEventsResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def list(
        self,
        *,
        at: Position | None = None,
        page: PageRequest | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> TraceEventsListResponse:
        request = TraceEventsListRequest(at=at, page=page)
        response = await unary(
            self._channel,
            "/harmos.host.v1.status.TraceEvents/List",
            request,
            TraceEventsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="status.trace_events.list",
        )
        return response

    async def watch(
        self,
        max_bytes: int,
        max_events: int,
        *,
        at: Position | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> AsyncGenerator[TraceEventsWatchResponse]:
        request = TraceEventsWatchRequest(at=at, max_bytes=max_bytes, max_events=max_events)
        call = stream(
            self._channel,
            "/harmos.host.v1.status.TraceEvents/Watch",
            request,
            TraceEventsWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )
        try:
            async for response in call:
                yield response
        except AioRpcError as error:
            raise call_error("status.trace_events.watch", error) from error
        finally:
            call.cancel()


class AccessResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel
        self._library_certificates = AccessCertificatesResource(channel)
        self._library_listener = AccessListenerResource(channel)
        self._library_pairings = AccessPairingsResource(channel)

    @property
    def certificates(self) -> AccessCertificatesResource:
        return self._library_certificates

    @property
    def listener(self) -> AccessListenerResource:
        return self._library_listener

    @property
    def pairings(self) -> AccessPairingsResource:
        return self._library_pairings


class InvocationsResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def create(
        self,
        operation: str,
        fingerprint: bytes,
        payload: bytes,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> bytes:
        request = InvocationsCreateRequest(operation=operation, fingerprint=fingerprint, payload=payload)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Invocations/Create",
            request,
            InvocationsCreateResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="invocations.create",
        )
        return response.payload

    async def get(
        self, instance: int, *, metadata: Metadata | None = None, timeout: float | None = None, credentials: CallCredentials | None = None
    ) -> InstanceState:
        request = InvocationsGetRequest(instance=instance)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Invocations/Get",
            request,
            InvocationsGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="invocations.get",
        )
        if not response.HasField("invocation"):
            raise ValueError("Read Invocations.Get: Missing Payload")
        return response.invocation

    async def cancel(
        self,
        instance: int,
        reason: str,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> InstanceState:
        request = InvocationsCancelRequest(instance=instance, reason=reason)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Invocations/Cancel",
            request,
            InvocationsCancelResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="invocations.cancel",
        )
        if not response.HasField("invocation"):
            raise ValueError("Read Invocations.Cancel: Missing Payload")
        return response.invocation

    async def watch(
        self, instance: int, *, metadata: Metadata | None = None, timeout: float | None = None, credentials: CallCredentials | None = None
    ) -> AsyncGenerator[InstanceState]:
        request = InvocationsWatchRequest(instance=instance)
        call = stream(
            self._channel,
            "/harmos.host.v1.Invocations/Watch",
            request,
            InvocationsWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )
        try:
            async for response in call:
                if not response.HasField("invocation"):
                    raise ValueError("Read Invocations.Watch: Missing Payload")
                yield response.invocation
        except AioRpcError as error:
            raise call_error("invocations.watch", error) from error
        finally:
            call.cancel()


class OperationsResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def list(
        self, *, metadata: Metadata | None = None, timeout: float | None = None, credentials: CallCredentials | None = None
    ) -> list[DiscoveredOperation]:
        request = OperationsListRequest()
        response = await unary(
            self._channel,
            "/harmos.host.v1.Operations/List",
            request,
            OperationsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="operations.list",
        )
        return list(response.operations)

    async def get(
        self, id: str, *, metadata: Metadata | None = None, timeout: float | None = None, credentials: CallCredentials | None = None
    ) -> DiscoveredOperation:
        request = OperationsGetRequest(id=id)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Operations/Get",
            request,
            OperationsGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="operations.get",
        )
        if not response.HasField("operation"):
            raise ValueError("Read Operations.Get: Missing Payload")
        return response.operation


class PluginsResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel
        self._library_settings = PluginsSettingsResource(channel)

    @property
    def settings(self) -> PluginsSettingsResource:
        return self._library_settings

    async def get(
        self,
        name: str,
        *,
        at: ReadCut | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> PluginsGetResponse:
        request = PluginsGetRequest(name=name, at=at)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Plugins/Get",
            request,
            PluginsGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="plugins.get",
        )
        return response

    async def list(
        self,
        *,
        at: ReadCut | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> PluginsListResponse:
        request = PluginsListRequest(at=at)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Plugins/List",
            request,
            PluginsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="plugins.list",
        )
        return response

    async def watch(
        self,
        *,
        name: str | None = None,
        at: ReadCut | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> AsyncGenerator[PluginsWatchResponse]:
        request = PluginsWatchRequest(name=name, at=at)
        call = stream(
            self._channel,
            "/harmos.host.v1.Plugins/Watch",
            request,
            PluginsWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )
        try:
            async for response in call:
                yield response
        except AioRpcError as error:
            raise call_error("plugins.watch", error) from error
        finally:
            call.cancel()

    async def create(
        self,
        name: str,
        placement: PluginPlacement,
        value: PluginValue,
        *,
        request_id: str | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> PluginsCreateResponse:
        request = PluginsCreateRequest(
            request_id=request_id if request_id is not None else str(uuid4()), name=name, placement=placement, value=value
        )
        response = await unary(
            self._channel,
            "/harmos.host.v1.Plugins/Create",
            request,
            PluginsCreateResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="plugins.create",
        )
        return response

    async def set(
        self,
        name: str,
        value: PluginValue,
        expected_revision: bytes,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> PluginsSetResponse:
        request = PluginsSetRequest(name=name, value=value, expected_revision=expected_revision)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Plugins/Set",
            request,
            PluginsSetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="plugins.set",
        )
        return response

    async def delete(
        self,
        name: str,
        expected_revision: bytes,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> Receipt:
        request = PluginsDeleteRequest(name=name, expected_revision=expected_revision)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Plugins/Delete",
            request,
            PluginsDeleteResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="plugins.delete",
        )
        if not response.HasField("receipt"):
            raise ValueError("Read Plugins.Delete: Missing Payload")
        return response.receipt

    async def restart(
        self,
        name: str,
        expected_revision: bytes,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> PluginEntry:
        request = PluginsRestartRequest(name=name, expected_revision=expected_revision)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Plugins/Restart",
            request,
            PluginsRestartResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="plugins.restart",
        )
        if not response.HasField("entry"):
            raise ValueError("Read Plugins.Restart: Missing Payload")
        return response.entry

    async def replace(
        self,
        name: str,
        placement: PluginPlacement,
        expected_revision: bytes,
        *,
        request_id: str | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> PluginsReplaceResponse:
        request = PluginsReplaceRequest(
            name=name,
            placement=placement,
            expected_revision=expected_revision,
            request_id=request_id if request_id is not None else str(uuid4()),
        )
        response = await unary(
            self._channel,
            "/harmos.host.v1.Plugins/Replace",
            request,
            PluginsReplaceResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="plugins.replace",
        )
        return response


class SchemasResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def list(
        self,
        *,
        schema: str | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> list[SchemaDescription]:
        request = SchemasListRequest(schema=schema)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Schemas/List",
            request,
            SchemasListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="schemas.list",
        )
        return list(response.schemas)


class StatusResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel
        self._library_diagnostics = StatusDiagnosticsResource(channel)
        self._library_logs = StatusLogsResource(channel)
        self._library_trace_events = StatusTraceEventsResource(channel)

    @property
    def diagnostics(self) -> StatusDiagnosticsResource:
        return self._library_diagnostics

    @property
    def logs(self) -> StatusLogsResource:
        return self._library_logs

    @property
    def trace_events(self) -> StatusTraceEventsResource:
        return self._library_trace_events

    async def get(
        self, *, metadata: Metadata | None = None, timeout: float | None = None, credentials: CallCredentials | None = None
    ) -> StatusGetResponse:
        request = StatusGetRequest()
        response = await unary(
            self._channel,
            "/harmos.host.v1.Status/Get",
            request,
            StatusGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="status.get",
        )
        return response

    async def watch(
        self,
        *,
        at: Position | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> AsyncGenerator[StatusWatchResponse]:
        request = StatusWatchRequest(at=at)
        call = stream(
            self._channel,
            "/harmos.host.v1.Status/Watch",
            request,
            StatusWatchResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
        )
        try:
            async for response in call:
                yield response
        except AioRpcError as error:
            raise call_error("status.watch", error) from error
        finally:
            call.cancel()


class StreamsResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def list(
        self,
        *,
        schema: str | None = None,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> list[StreamIdentity]:
        request = StreamsListRequest(schema=schema)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Streams/List",
            request,
            StreamsListResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="streams.list",
        )
        return list(response.streams)


class TimeResource:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel

    async def get(
        self, *, metadata: Metadata | None = None, timeout: float | None = None, credentials: CallCredentials | None = None
    ) -> Timestamp:
        request = TimeGetRequest()
        response = await unary(
            self._channel,
            "/harmos.host.v1.Time/Get",
            request,
            TimeGetResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="time.get",
        )
        if not response.HasField("now"):
            raise ValueError("Read Time.Get: Missing Payload")
        return response.now

    async def sleep(
        self,
        duration: Duration,
        *,
        metadata: Metadata | None = None,
        timeout: float | None = None,
        credentials: CallCredentials | None = None,
    ) -> TimeSleepResponse:
        request = TimeSleepRequest(duration=duration)
        response = await unary(
            self._channel,
            "/harmos.host.v1.Time/Sleep",
            request,
            TimeSleepResponse,
            metadata=metadata,
            timeout=timeout,
            credentials=credentials,
            action="time.sleep",
        )
        return response


class ClientRoot:
    def __init__(self, channel: Channel) -> None:
        self._channel = channel
        self._library_access = AccessResource(channel)
        self._library_invocations = InvocationsResource(channel)
        self._library_operations = OperationsResource(channel)
        self._library_plugins = PluginsResource(channel)
        self._library_schemas = SchemasResource(channel)
        self._library_status = StatusResource(channel)
        self._library_streams = StreamsResource(channel)
        self._library_time = TimeResource(channel)

    @property
    def access(self) -> AccessResource:
        return self._library_access

    @property
    def invocations(self) -> InvocationsResource:
        return self._library_invocations

    @property
    def operations(self) -> OperationsResource:
        return self._library_operations

    @property
    def plugins(self) -> PluginsResource:
        return self._library_plugins

    @property
    def schemas(self) -> SchemasResource:
        return self._library_schemas

    @property
    def status(self) -> StatusResource:
        return self._library_status

    @property
    def streams(self) -> StreamsResource:
        return self._library_streams

    @property
    def time(self) -> TimeResource:
        return self._library_time
