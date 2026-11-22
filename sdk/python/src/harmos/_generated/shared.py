# @generated from protocol descriptors; do not edit.
from harmos._wire.model.v1.callers_pb2 import ALL, CALLERS_UNSPECIFIED, CLIENTS, PLUGINS, Callers
from harmos._wire.model.v1.description_pb2 import Declarations, LaneDeclaration, Section
from harmos._wire.model.v1.failure_pb2 import (
    CLASSIFICATION_CANCELLED,
    CLASSIFICATION_INVALID,
    CLASSIFICATION_PERMANENT,
    CLASSIFICATION_REFUSED,
    CLASSIFICATION_TIMED_OUT,
    CLASSIFICATION_TRANSIENT,
    CLASSIFICATION_UNSPECIFIED,
    Classification,
)
from harmos._wire.model.v1.file_pb2 import (
    UPLOAD_STATE_ABANDONED,
    UPLOAD_STATE_FINISHED,
    UPLOAD_STATE_PRODUCING,
    UPLOAD_STATE_UNSPECIFIED,
    File,
    FileMetadata,
    FileReference,
    ListFileProductionsRequest,
    ListFileProductionsResponse,
    Upload,
    UploadState,
)
from harmos._wire.model.v1.invocation_pb2 import InstanceState
from harmos._wire.model.v1.operation_pb2 import (
    VISIBILITY_PRIVATE,
    VISIBILITY_PROTECTED,
    VISIBILITY_PUBLIC,
    VISIBILITY_UNSPECIFIED,
    DiscoveredContract,
    DiscoveredOperation,
    Grant,
    Grpc,
    OperationContract,
    Protobuf,
    Requirement,
    Visibility,
)
from harmos._wire.model.v1.page_pb2 import PageRequest
from harmos._wire.model.v1.policy_pb2 import OperationPolicy, RetryPolicy
from harmos._wire.model.v1.position_pb2 import History, Position, ReadCut, ResourceRevision
from harmos._wire.model.v1.receipt_pb2 import Receipt
from harmos._wire.model.v1.setting_pb2 import SettingDeclaration, SettingsDeclarations, SettingValue, SettingValues
from harmos._wire.model.v1.stream_pb2 import StreamDeclaration
from harmos._wire.model.v1.transfer_pb2 import (
    TRANSFER_FRAMING_BYTES,
    TRANSFER_FRAMING_UNSPECIFIED,
    MessageSettings,
    StorageTransferSettings,
    TransferFraming,
)

from ._exports.shared import model

Declarations.__module__ = "harmos"
Declarations.__qualname__ = "Declarations"
LaneDeclaration.__module__ = "harmos"
LaneDeclaration.__qualname__ = "LaneDeclaration"
Section.__module__ = "harmos"
Section.__qualname__ = "Section"
File.__module__ = "harmos"
File.__qualname__ = "File"
FileMetadata.__module__ = "harmos"
FileMetadata.__qualname__ = "FileMetadata"
FileReference.__module__ = "harmos"
FileReference.__qualname__ = "FileReference"
ListFileProductionsRequest.__module__ = "harmos"
ListFileProductionsRequest.__qualname__ = "ListFileProductionsRequest"
ListFileProductionsResponse.__module__ = "harmos"
ListFileProductionsResponse.__qualname__ = "ListFileProductionsResponse"
Upload.__module__ = "harmos"
Upload.__qualname__ = "Upload"
InstanceState.__module__ = "harmos"
InstanceState.__qualname__ = "InstanceState"
DiscoveredContract.__module__ = "harmos"
DiscoveredContract.__qualname__ = "DiscoveredContract"
DiscoveredOperation.__module__ = "harmos"
DiscoveredOperation.__qualname__ = "DiscoveredOperation"
Grant.__module__ = "harmos"
Grant.__qualname__ = "Grant"
Grpc.__module__ = "harmos"
Grpc.__qualname__ = "Grpc"
OperationContract.__module__ = "harmos"
OperationContract.__qualname__ = "OperationContract"
Protobuf.__module__ = "harmos"
Protobuf.__qualname__ = "Protobuf"
Requirement.__module__ = "harmos"
Requirement.__qualname__ = "Requirement"
PageRequest.__module__ = "harmos"
PageRequest.__qualname__ = "PageRequest"
OperationPolicy.__module__ = "harmos"
OperationPolicy.__qualname__ = "OperationPolicy"
RetryPolicy.__module__ = "harmos"
RetryPolicy.__qualname__ = "RetryPolicy"
History.__module__ = "harmos"
History.__qualname__ = "History"
Position.__module__ = "harmos"
Position.__qualname__ = "Position"
ReadCut.__module__ = "harmos"
ReadCut.__qualname__ = "ReadCut"
ResourceRevision.__module__ = "harmos"
ResourceRevision.__qualname__ = "ResourceRevision"
Receipt.__module__ = "harmos"
Receipt.__qualname__ = "Receipt"
SettingDeclaration.__module__ = "harmos"
SettingDeclaration.__qualname__ = "SettingDeclaration"
SettingValue.__module__ = "harmos"
SettingValue.__qualname__ = "SettingValue"
SettingValues.__module__ = "harmos"
SettingValues.__qualname__ = "SettingValues"
SettingsDeclarations.__module__ = "harmos"
SettingsDeclarations.__qualname__ = "SettingsDeclarations"
StreamDeclaration.__module__ = "harmos"
StreamDeclaration.__qualname__ = "StreamDeclaration"
MessageSettings.__module__ = "harmos"
MessageSettings.__qualname__ = "MessageSettings"
StorageTransferSettings.__module__ = "harmos"
StorageTransferSettings.__qualname__ = "StorageTransferSettings"

__all__ = [
    "ALL",
    "CALLERS_UNSPECIFIED",
    "CLASSIFICATION_CANCELLED",
    "CLASSIFICATION_INVALID",
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
    "File",
    "FileMetadata",
    "FileReference",
    "Grant",
    "Grpc",
    "History",
    "InstanceState",
    "LaneDeclaration",
    "ListFileProductionsRequest",
    "ListFileProductionsResponse",
    "MessageSettings",
    "OperationContract",
    "OperationPolicy",
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
    "StorageTransferSettings",
    "StreamDeclaration",
    "TRANSFER_FRAMING_BYTES",
    "TRANSFER_FRAMING_UNSPECIFIED",
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
    "model",
]
