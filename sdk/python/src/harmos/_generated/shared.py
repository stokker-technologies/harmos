# @generated from protocol descriptors; do not edit.
from harmos._wire.model.v1.callers_pb2 import ALL, CALLERS_UNSPECIFIED, CLIENTS, PLUGINS, Callers
from harmos._wire.model.v1.description_pb2 import Declarations, Section
from harmos._wire.model.v1.failure_pb2 import (
    CLASSIFICATION_CANCELLED,
    CLASSIFICATION_PERMANENT,
    CLASSIFICATION_REFUSED,
    CLASSIFICATION_TIMED_OUT,
    CLASSIFICATION_TRANSIENT,
    CLASSIFICATION_UNSPECIFIED,
    Classification,
)
from harmos._wire.model.v1.failure_pb2 import Failure as FailureValue
from harmos._wire.model.v1.file_pb2 import (
    UPLOAD_STATE_ABANDONED,
    UPLOAD_STATE_FINISHED,
    UPLOAD_STATE_PRODUCING,
    UPLOAD_STATE_UNSPECIFIED,
    File,
    FileMetadata,
    FileReference,
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
]
