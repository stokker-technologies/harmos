# @generated from protocol descriptors; do not edit.
from harmos._wire.host.v1.invocations_pb2 import InvocationsCancelResponse as InvocationsCancelResponse
from harmos._wire.host.v1.invocations_pb2 import InvocationsGetResponse as InvocationsGetResponse
from harmos._wire.host.v1.invocations_pb2 import InvocationsWatchResponse as InvocationsWatchResponse
from harmos._wire.host.v1.operations_pb2 import OperationsGetResponse as OperationsGetResponse
from harmos._wire.host.v1.operations_pb2 import OperationsListResponse as OperationsListResponse
from harmos._wire.host.v1.plugins.settings_pb2 import SettingsListResponse as SettingsListResponse
from harmos._wire.host.v1.time_pb2 import TimeGetResponse as TimeGetResponse
from harmos._wire.host.v1.time_pb2 import TimeSleepResponse as TimeSleepResponse
from harmos._wire.model.v1.callers_pb2 import ALL as ALL
from harmos._wire.model.v1.callers_pb2 import CALLERS_UNSPECIFIED as CALLERS_UNSPECIFIED
from harmos._wire.model.v1.callers_pb2 import CLIENTS as CLIENTS
from harmos._wire.model.v1.callers_pb2 import PLUGINS as PLUGINS
from harmos._wire.model.v1.callers_pb2 import Callers as Callers
from harmos._wire.model.v1.description_pb2 import Declarations as Declarations
from harmos._wire.model.v1.description_pb2 import Section as Section
from harmos._wire.model.v1.failure_pb2 import CLASSIFICATION_CANCELLED as CLASSIFICATION_CANCELLED
from harmos._wire.model.v1.failure_pb2 import CLASSIFICATION_PERMANENT as CLASSIFICATION_PERMANENT
from harmos._wire.model.v1.failure_pb2 import CLASSIFICATION_REFUSED as CLASSIFICATION_REFUSED
from harmos._wire.model.v1.failure_pb2 import CLASSIFICATION_TIMED_OUT as CLASSIFICATION_TIMED_OUT
from harmos._wire.model.v1.failure_pb2 import CLASSIFICATION_TRANSIENT as CLASSIFICATION_TRANSIENT
from harmos._wire.model.v1.failure_pb2 import CLASSIFICATION_UNSPECIFIED as CLASSIFICATION_UNSPECIFIED
from harmos._wire.model.v1.failure_pb2 import Classification as Classification
from harmos._wire.model.v1.failure_pb2 import Failure as FailureValue
from harmos._wire.model.v1.file_pb2 import UPLOAD_STATE_ABANDONED as UPLOAD_STATE_ABANDONED
from harmos._wire.model.v1.file_pb2 import UPLOAD_STATE_FINISHED as UPLOAD_STATE_FINISHED
from harmos._wire.model.v1.file_pb2 import UPLOAD_STATE_PRODUCING as UPLOAD_STATE_PRODUCING
from harmos._wire.model.v1.file_pb2 import UPLOAD_STATE_UNSPECIFIED as UPLOAD_STATE_UNSPECIFIED
from harmos._wire.model.v1.file_pb2 import File as File
from harmos._wire.model.v1.file_pb2 import FileMetadata as FileMetadata
from harmos._wire.model.v1.file_pb2 import FileReference as FileReference
from harmos._wire.model.v1.file_pb2 import Upload as Upload
from harmos._wire.model.v1.file_pb2 import UploadState as UploadState
from harmos._wire.model.v1.invocation_pb2 import InstanceState as InstanceState
from harmos._wire.model.v1.operation_pb2 import VISIBILITY_PRIVATE as VISIBILITY_PRIVATE
from harmos._wire.model.v1.operation_pb2 import VISIBILITY_PROTECTED as VISIBILITY_PROTECTED
from harmos._wire.model.v1.operation_pb2 import VISIBILITY_PUBLIC as VISIBILITY_PUBLIC
from harmos._wire.model.v1.operation_pb2 import VISIBILITY_UNSPECIFIED as VISIBILITY_UNSPECIFIED
from harmos._wire.model.v1.operation_pb2 import DiscoveredContract as DiscoveredContract
from harmos._wire.model.v1.operation_pb2 import DiscoveredOperation as DiscoveredOperation
from harmos._wire.model.v1.operation_pb2 import Grant as Grant
from harmos._wire.model.v1.operation_pb2 import Grpc as Grpc
from harmos._wire.model.v1.operation_pb2 import OperationContract as OperationContract
from harmos._wire.model.v1.operation_pb2 import Protobuf as Protobuf
from harmos._wire.model.v1.operation_pb2 import Requirement as Requirement
from harmos._wire.model.v1.operation_pb2 import Visibility as Visibility
from harmos._wire.model.v1.page_pb2 import PageRequest as PageRequest
from harmos._wire.model.v1.policy_pb2 import OperationPolicy as OperationPolicy
from harmos._wire.model.v1.policy_pb2 import RetryPolicy as RetryPolicy
from harmos._wire.model.v1.position_pb2 import History as History
from harmos._wire.model.v1.position_pb2 import Position as Position
from harmos._wire.model.v1.position_pb2 import ReadCut as ReadCut
from harmos._wire.model.v1.position_pb2 import ResourceRevision as ResourceRevision
from harmos._wire.model.v1.receipt_pb2 import Receipt as Receipt
from harmos._wire.model.v1.setting_pb2 import SettingDeclaration as SettingDeclaration
from harmos._wire.model.v1.setting_pb2 import SettingsDeclarations as SettingsDeclarations
from harmos._wire.model.v1.setting_pb2 import SettingValue as SettingValue
from harmos._wire.model.v1.setting_pb2 import SettingValues as SettingValues
from harmos._wire.model.v1.stream_pb2 import StreamDeclaration as StreamDeclaration
from harmos._wire.model.v1.transfer_pb2 import TRANSFER_FRAMING_BYTES as TRANSFER_FRAMING_BYTES
from harmos._wire.model.v1.transfer_pb2 import TRANSFER_FRAMING_UNSPECIFIED as TRANSFER_FRAMING_UNSPECIFIED
from harmos._wire.model.v1.transfer_pb2 import MessageSettings as MessageSettings
from harmos._wire.model.v1.transfer_pb2 import StorageTransferSettings as StorageTransferSettings
from harmos._wire.model.v1.transfer_pb2 import TransferFraming as TransferFraming

InvocationsCancelResponse.__module__ = "harmos"
InvocationsGetResponse.__module__ = "harmos"
InvocationsWatchResponse.__module__ = "harmos"
OperationsGetResponse.__module__ = "harmos"
OperationsListResponse.__module__ = "harmos"
SettingsListResponse.__module__ = "harmos"
TimeGetResponse.__module__ = "harmos"
TimeSleepResponse.__module__ = "harmos"
Declarations.__module__ = "harmos"
Section.__module__ = "harmos"
FailureValue.__module__ = "harmos"
File.__module__ = "harmos"
FileMetadata.__module__ = "harmos"
FileReference.__module__ = "harmos"
Upload.__module__ = "harmos"
InstanceState.__module__ = "harmos"
DiscoveredContract.__module__ = "harmos"
DiscoveredOperation.__module__ = "harmos"
Grant.__module__ = "harmos"
Grpc.__module__ = "harmos"
OperationContract.__module__ = "harmos"
Protobuf.__module__ = "harmos"
Requirement.__module__ = "harmos"
PageRequest.__module__ = "harmos"
OperationPolicy.__module__ = "harmos"
RetryPolicy.__module__ = "harmos"
History.__module__ = "harmos"
Position.__module__ = "harmos"
ReadCut.__module__ = "harmos"
ResourceRevision.__module__ = "harmos"
Receipt.__module__ = "harmos"
SettingDeclaration.__module__ = "harmos"
SettingValue.__module__ = "harmos"
SettingValues.__module__ = "harmos"
SettingsDeclarations.__module__ = "harmos"
StreamDeclaration.__module__ = "harmos"
MessageSettings.__module__ = "harmos"
StorageTransferSettings.__module__ = "harmos"

__all__ = [
    "ALL",
    "CALLERS_UNSPECIFIED",
    "CLASSIFICATION_CANCELLED",
    "CLASSIFICATION_PERMANENT",
    "CLASSIFICATION_REFUSED",
    "CLASSIFICATION_TIMED_OUT",
    "CLASSIFICATION_TRANSIENT",
    "CLASSIFICATION_UNSPECIFIED",
    "CLIENTS",
    "Callers",
    "Classification",
    "Declarations",
    "DiscoveredContract",
    "DiscoveredOperation",
    "FailureValue",
    "File",
    "FileMetadata",
    "FileReference",
    "Grant",
    "Grpc",
    "History",
    "InstanceState",
    "InvocationsCancelResponse",
    "InvocationsGetResponse",
    "InvocationsWatchResponse",
    "MessageSettings",
    "OperationContract",
    "OperationPolicy",
    "OperationsGetResponse",
    "OperationsListResponse",
    "PLUGINS",
    "PageRequest",
    "Position",
    "Protobuf",
    "ReadCut",
    "Receipt",
    "Requirement",
    "ResourceRevision",
    "RetryPolicy",
    "Section",
    "SettingDeclaration",
    "SettingValue",
    "SettingValues",
    "SettingsDeclarations",
    "SettingsListResponse",
    "StorageTransferSettings",
    "StreamDeclaration",
    "TRANSFER_FRAMING_BYTES",
    "TRANSFER_FRAMING_UNSPECIFIED",
    "TimeGetResponse",
    "TimeSleepResponse",
    "TransferFraming",
    "UPLOAD_STATE_ABANDONED",
    "UPLOAD_STATE_FINISHED",
    "UPLOAD_STATE_PRODUCING",
    "UPLOAD_STATE_UNSPECIFIED",
    "Upload",
    "UploadState",
    "VISIBILITY_PRIVATE",
    "VISIBILITY_PROTECTED",
    "VISIBILITY_PUBLIC",
    "VISIBILITY_UNSPECIFIED",
    "Visibility",
]
