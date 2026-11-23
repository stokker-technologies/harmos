from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Optional as _Optional

DESCRIPTOR: _descriptor.FileDescriptor

class InstanceState(_message.Message):
    __slots__ = ("phase", "outcome", "cancellation_requested", "attempt")
    PHASE_FIELD_NUMBER: _ClassVar[int]
    OUTCOME_FIELD_NUMBER: _ClassVar[int]
    CANCELLATION_REQUESTED_FIELD_NUMBER: _ClassVar[int]
    ATTEMPT_FIELD_NUMBER: _ClassVar[int]
    phase: str
    outcome: str
    cancellation_requested: bool
    attempt: int
    def __init__(self, phase: _Optional[str] = ..., outcome: _Optional[str] = ..., cancellation_requested: _Optional[bool] = ..., attempt: _Optional[int] = ...) -> None: ...
