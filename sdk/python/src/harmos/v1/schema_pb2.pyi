from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class PayloadEncoding(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PAYLOAD_ENCODING_UNSPECIFIED: _ClassVar[PayloadEncoding]
    POSTCARD_V1: _ClassVar[PayloadEncoding]

class SchemaCardinality(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    REQUIRED: _ClassVar[SchemaCardinality]
    OPTIONAL: _ClassVar[SchemaCardinality]
    REPEATED: _ClassVar[SchemaCardinality]
    VARIANT: _ClassVar[SchemaCardinality]

class SchemaKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    BOOL: _ClassVar[SchemaKind]
    UINT32: _ClassVar[SchemaKind]
    UINT64: _ClassVar[SchemaKind]
    INT32: _ClassVar[SchemaKind]
    INT64: _ClassVar[SchemaKind]
    FLOAT32: _ClassVar[SchemaKind]
    FLOAT64: _ClassVar[SchemaKind]
    STRING: _ClassVar[SchemaKind]
    BYTES: _ClassVar[SchemaKind]
    MESSAGE: _ClassVar[SchemaKind]

class SchemaForm(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    OBJECT: _ClassVar[SchemaForm]
    UNIT: _ClassVar[SchemaForm]
    VALUE: _ClassVar[SchemaForm]
    TUPLE: _ClassVar[SchemaForm]
    EXTERNAL: _ClassVar[SchemaForm]
    INTERNAL: _ClassVar[SchemaForm]
    ADJACENT: _ClassVar[SchemaForm]
    UNTAGGED: _ClassVar[SchemaForm]

class SchemaOmission(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    OMIT_NONE: _ClassVar[SchemaOmission]
    OMIT_EMPTY: _ClassVar[SchemaOmission]
PAYLOAD_ENCODING_UNSPECIFIED: PayloadEncoding
POSTCARD_V1: PayloadEncoding
REQUIRED: SchemaCardinality
OPTIONAL: SchemaCardinality
REPEATED: SchemaCardinality
VARIANT: SchemaCardinality
BOOL: SchemaKind
UINT32: SchemaKind
UINT64: SchemaKind
INT32: SchemaKind
INT64: SchemaKind
FLOAT32: SchemaKind
FLOAT64: SchemaKind
STRING: SchemaKind
BYTES: SchemaKind
MESSAGE: SchemaKind
OBJECT: SchemaForm
UNIT: SchemaForm
VALUE: SchemaForm
TUPLE: SchemaForm
EXTERNAL: SchemaForm
INTERNAL: SchemaForm
ADJACENT: SchemaForm
UNTAGGED: SchemaForm
OMIT_NONE: SchemaOmission
OMIT_EMPTY: SchemaOmission

class SchemaKey(_message.Message):
    __slots__ = ("id", "version", "payload_encoding")
    ID_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_ENCODING_FIELD_NUMBER: _ClassVar[int]
    id: str
    version: int
    payload_encoding: PayloadEncoding
    def __init__(self, id: _Optional[str] = ..., version: _Optional[int] = ..., payload_encoding: _Optional[_Union[PayloadEncoding, str]] = ...) -> None: ...

class SchemaField(_message.Message):
    __slots__ = ("name", "tag", "kind", "cardinality", "definition")
    NAME_FIELD_NUMBER: _ClassVar[int]
    TAG_FIELD_NUMBER: _ClassVar[int]
    KIND_FIELD_NUMBER: _ClassVar[int]
    CARDINALITY_FIELD_NUMBER: _ClassVar[int]
    DEFINITION_FIELD_NUMBER: _ClassVar[int]
    name: str
    tag: int
    kind: SchemaKind
    cardinality: SchemaCardinality
    definition: int
    def __init__(self, name: _Optional[str] = ..., tag: _Optional[int] = ..., kind: _Optional[_Union[SchemaKind, str]] = ..., cardinality: _Optional[_Union[SchemaCardinality, str]] = ..., definition: _Optional[int] = ...) -> None: ...

class SchemaDefinition(_message.Message):
    __slots__ = ("fields",)
    FIELDS_FIELD_NUMBER: _ClassVar[int]
    fields: _containers.RepeatedCompositeFieldContainer[SchemaField]
    def __init__(self, fields: _Optional[_Iterable[_Union[SchemaField, _Mapping]]] = ...) -> None: ...

class SchemaRepresentation(_message.Message):
    __slots__ = ("definition", "form", "tag", "content", "omissions", "maps")
    class OmissionsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: SchemaOmission
        def __init__(self, key: _Optional[str] = ..., value: _Optional[_Union[SchemaOmission, str]] = ...) -> None: ...
    DEFINITION_FIELD_NUMBER: _ClassVar[int]
    FORM_FIELD_NUMBER: _ClassVar[int]
    TAG_FIELD_NUMBER: _ClassVar[int]
    CONTENT_FIELD_NUMBER: _ClassVar[int]
    OMISSIONS_FIELD_NUMBER: _ClassVar[int]
    MAPS_FIELD_NUMBER: _ClassVar[int]
    definition: int
    form: SchemaForm
    tag: str
    content: str
    omissions: _containers.ScalarMap[str, SchemaOmission]
    maps: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, definition: _Optional[int] = ..., form: _Optional[_Union[SchemaForm, str]] = ..., tag: _Optional[str] = ..., content: _Optional[str] = ..., omissions: _Optional[_Mapping[str, SchemaOmission]] = ..., maps: _Optional[_Iterable[str]] = ...) -> None: ...

class SchemaGraph(_message.Message):
    __slots__ = ("root", "definitions", "representations")
    ROOT_FIELD_NUMBER: _ClassVar[int]
    DEFINITIONS_FIELD_NUMBER: _ClassVar[int]
    REPRESENTATIONS_FIELD_NUMBER: _ClassVar[int]
    root: int
    definitions: _containers.RepeatedCompositeFieldContainer[SchemaDefinition]
    representations: _containers.RepeatedCompositeFieldContainer[SchemaRepresentation]
    def __init__(self, root: _Optional[int] = ..., definitions: _Optional[_Iterable[_Union[SchemaDefinition, _Mapping]]] = ..., representations: _Optional[_Iterable[_Union[SchemaRepresentation, _Mapping]]] = ...) -> None: ...

class SchemaInput(_message.Message):
    __slots__ = ("key", "graph")
    KEY_FIELD_NUMBER: _ClassVar[int]
    GRAPH_FIELD_NUMBER: _ClassVar[int]
    key: SchemaKey
    graph: SchemaGraph
    def __init__(self, key: _Optional[_Union[SchemaKey, _Mapping]] = ..., graph: _Optional[_Union[SchemaGraph, _Mapping]] = ...) -> None: ...
