from harmos._wire.model.v1 import description_pb2 as _description_pb2
from harmos._wire.model.v1 import policy_pb2 as _policy_pb2
from harmos._wire.plugin.v1 import invocation_pb2 as _invocation_pb2
from harmos._wire.plugin.v1 import operation_pb2 as _operation_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class GuestChildAction(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    GUEST_CHILD_ACTION_UNSPECIFIED: _ClassVar[GuestChildAction]
    GUEST_CHILD_ACTION_STATUS: _ClassVar[GuestChildAction]
    GUEST_CHILD_ACTION_CANCEL: _ClassVar[GuestChildAction]
    GUEST_CHILD_ACTION_JOIN: _ClassVar[GuestChildAction]
    GUEST_CHILD_ACTION_RELEASE: _ClassVar[GuestChildAction]
    GUEST_CHILD_ACTION_DETACH: _ClassVar[GuestChildAction]
GUEST_CHILD_ACTION_UNSPECIFIED: GuestChildAction
GUEST_CHILD_ACTION_STATUS: GuestChildAction
GUEST_CHILD_ACTION_CANCEL: GuestChildAction
GUEST_CHILD_ACTION_JOIN: GuestChildAction
GUEST_CHILD_ACTION_RELEASE: GuestChildAction
GUEST_CHILD_ACTION_DETACH: GuestChildAction

class GuestRegistration(_message.Message):
    __slots__ = ("id", "version", "declarations", "initialization", "shutdown", "requirements", "message_bytes")
    ID_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    DECLARATIONS_FIELD_NUMBER: _ClassVar[int]
    INITIALIZATION_FIELD_NUMBER: _ClassVar[int]
    SHUTDOWN_FIELD_NUMBER: _ClassVar[int]
    REQUIREMENTS_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_BYTES_FIELD_NUMBER: _ClassVar[int]
    id: str
    version: str
    declarations: _description_pb2.Declarations
    initialization: _policy_pb2.OperationPolicy
    shutdown: _policy_pb2.OperationPolicy
    requirements: _containers.RepeatedCompositeFieldContainer[GuestRequirement]
    message_bytes: int
    def __init__(self, id: _Optional[str] = ..., version: _Optional[str] = ..., declarations: _Optional[_Union[_description_pb2.Declarations, _Mapping]] = ..., initialization: _Optional[_Union[_policy_pb2.OperationPolicy, _Mapping]] = ..., shutdown: _Optional[_Union[_policy_pb2.OperationPolicy, _Mapping]] = ..., requirements: _Optional[_Iterable[_Union[GuestRequirement, _Mapping]]] = ..., message_bytes: _Optional[int] = ...) -> None: ...

class GuestRequirement(_message.Message):
    __slots__ = ("id", "request", "response", "fingerprint")
    ID_FIELD_NUMBER: _ClassVar[int]
    REQUEST_FIELD_NUMBER: _ClassVar[int]
    RESPONSE_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    id: str
    request: str
    response: str
    fingerprint: bytes
    def __init__(self, id: _Optional[str] = ..., request: _Optional[str] = ..., response: _Optional[str] = ..., fingerprint: _Optional[bytes] = ...) -> None: ...

class GuestCommand(_message.Message):
    __slots__ = ("prepare", "initialize", "attempt", "finalize", "cancel")
    PREPARE_FIELD_NUMBER: _ClassVar[int]
    INITIALIZE_FIELD_NUMBER: _ClassVar[int]
    ATTEMPT_FIELD_NUMBER: _ClassVar[int]
    FINALIZE_FIELD_NUMBER: _ClassVar[int]
    CANCEL_FIELD_NUMBER: _ClassVar[int]
    prepare: GuestPrepare
    initialize: GuestInvocation
    attempt: GuestInvocation
    finalize: int
    cancel: int
    def __init__(self, prepare: _Optional[_Union[GuestPrepare, _Mapping]] = ..., initialize: _Optional[_Union[GuestInvocation, _Mapping]] = ..., attempt: _Optional[_Union[GuestInvocation, _Mapping]] = ..., finalize: _Optional[int] = ..., cancel: _Optional[int] = ...) -> None: ...

class GuestPrepare(_message.Message):
    __slots__ = ("invocation", "operation", "fingerprint", "generation", "payload")
    INVOCATION_FIELD_NUMBER: _ClassVar[int]
    OPERATION_FIELD_NUMBER: _ClassVar[int]
    FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    GENERATION_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    invocation: int
    operation: str
    fingerprint: bytes
    generation: int
    payload: bytes
    def __init__(self, invocation: _Optional[int] = ..., operation: _Optional[str] = ..., fingerprint: _Optional[bytes] = ..., generation: _Optional[int] = ..., payload: _Optional[bytes] = ...) -> None: ...

class GuestInvocation(_message.Message):
    __slots__ = ("invocation", "context", "values")
    INVOCATION_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    VALUES_FIELD_NUMBER: _ClassVar[int]
    invocation: int
    context: bytes
    values: _containers.RepeatedCompositeFieldContainer[GuestValue]
    def __init__(self, invocation: _Optional[int] = ..., context: _Optional[bytes] = ..., values: _Optional[_Iterable[_Union[GuestValue, _Mapping]]] = ...) -> None: ...

class GuestValue(_message.Message):
    __slots__ = ("key", "payload")
    KEY_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    key: str
    payload: bytes
    def __init__(self, key: _Optional[str] = ..., payload: _Optional[bytes] = ...) -> None: ...

class GuestInitialize(_message.Message):
    __slots__ = ("configuration", "generation_context", "assignments")
    CONFIGURATION_FIELD_NUMBER: _ClassVar[int]
    GENERATION_CONTEXT_FIELD_NUMBER: _ClassVar[int]
    ASSIGNMENTS_FIELD_NUMBER: _ClassVar[int]
    configuration: bytes
    generation_context: bytes
    assignments: _operation_pb2.OperationAssignments
    def __init__(self, configuration: _Optional[bytes] = ..., generation_context: _Optional[bytes] = ..., assignments: _Optional[_Union[_operation_pb2.OperationAssignments, _Mapping]] = ...) -> None: ...

class GuestShutdown(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class GuestChild(_message.Message):
    __slots__ = ("call", "control")
    CALL_FIELD_NUMBER: _ClassVar[int]
    CONTROL_FIELD_NUMBER: _ClassVar[int]
    call: GuestChildRequest
    control: GuestChildControl
    def __init__(self, call: _Optional[_Union[GuestChildRequest, _Mapping]] = ..., control: _Optional[_Union[GuestChildControl, _Mapping]] = ...) -> None: ...

class GuestChildRequest(_message.Message):
    __slots__ = ("context", "generation_context", "request", "generation", "submit", "values")
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    GENERATION_CONTEXT_FIELD_NUMBER: _ClassVar[int]
    REQUEST_FIELD_NUMBER: _ClassVar[int]
    GENERATION_FIELD_NUMBER: _ClassVar[int]
    SUBMIT_FIELD_NUMBER: _ClassVar[int]
    VALUES_FIELD_NUMBER: _ClassVar[int]
    context: bytes
    generation_context: bytes
    request: _invocation_pb2.ChildRequest
    generation: int
    submit: bool
    values: _containers.RepeatedCompositeFieldContainer[GuestValue]
    def __init__(self, context: _Optional[bytes] = ..., generation_context: _Optional[bytes] = ..., request: _Optional[_Union[_invocation_pb2.ChildRequest, _Mapping]] = ..., generation: _Optional[int] = ..., submit: _Optional[bool] = ..., values: _Optional[_Iterable[_Union[GuestValue, _Mapping]]] = ...) -> None: ...

class GuestChildControl(_message.Message):
    __slots__ = ("context", "instance", "action")
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    INSTANCE_FIELD_NUMBER: _ClassVar[int]
    ACTION_FIELD_NUMBER: _ClassVar[int]
    context: bytes
    instance: int
    action: GuestChildAction
    def __init__(self, context: _Optional[bytes] = ..., instance: _Optional[int] = ..., action: _Optional[_Union[GuestChildAction, str]] = ...) -> None: ...

class GuestSubmitted(_message.Message):
    __slots__ = ("instance",)
    INSTANCE_FIELD_NUMBER: _ClassVar[int]
    instance: int
    def __init__(self, instance: _Optional[int] = ...) -> None: ...
