# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# source: ommx/v1/parameter.proto
# Protobuf Python Version: 5.26.1
"""Generated protocol buffer code."""

from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()


DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(
    b'\n\x17ommx/v1/parameter.proto\x12\x07ommx.v1"\x97\x02\n\tParameter\x12\x0e\n\x02id\x18\x01 \x01(\x04R\x02id\x12\x17\n\x04name\x18\x02 \x01(\tH\x00R\x04name\x88\x01\x01\x12\x1e\n\nsubscripts\x18\x03 \x03(\x03R\nsubscripts\x12\x42\n\nparameters\x18\x04 \x03(\x0b\x32".ommx.v1.Parameter.ParametersEntryR\nparameters\x12%\n\x0b\x64\x65scription\x18\x05 \x01(\tH\x01R\x0b\x64\x65scription\x88\x01\x01\x1a=\n\x0fParametersEntry\x12\x10\n\x03key\x18\x01 \x01(\tR\x03key\x12\x14\n\x05value\x18\x02 \x01(\tR\x05value:\x02\x38\x01\x42\x07\n\x05_nameB\x0e\n\x0c_description"\x84\x01\n\nParameters\x12:\n\x07\x65ntries\x18\x01 \x03(\x0b\x32 .ommx.v1.Parameters.EntriesEntryR\x07\x65ntries\x1a:\n\x0c\x45ntriesEntry\x12\x10\n\x03key\x18\x01 \x01(\x04R\x03key\x12\x14\n\x05value\x18\x02 \x01(\x01R\x05value:\x02\x38\x01\x42Z\n\x0b\x63om.ommx.v1B\x0eParameterProtoP\x01\xa2\x02\x03OXX\xaa\x02\x07Ommx.V1\xca\x02\x07Ommx\\V1\xe2\x02\x13Ommx\\V1\\GPBMetadata\xea\x02\x08Ommx::V1b\x06proto3'
)

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, "ommx.v1.parameter_pb2", _globals)
if not _descriptor._USE_C_DESCRIPTORS:
    _globals["DESCRIPTOR"]._loaded_options = None
    _globals[
        "DESCRIPTOR"
    ]._serialized_options = b"\n\013com.ommx.v1B\016ParameterProtoP\001\242\002\003OXX\252\002\007Ommx.V1\312\002\007Ommx\\V1\342\002\023Ommx\\V1\\GPBMetadata\352\002\010Ommx::V1"
    _globals["_PARAMETER_PARAMETERSENTRY"]._loaded_options = None
    _globals["_PARAMETER_PARAMETERSENTRY"]._serialized_options = b"8\001"
    _globals["_PARAMETERS_ENTRIESENTRY"]._loaded_options = None
    _globals["_PARAMETERS_ENTRIESENTRY"]._serialized_options = b"8\001"
    _globals["_PARAMETER"]._serialized_start = 37
    _globals["_PARAMETER"]._serialized_end = 316
    _globals["_PARAMETER_PARAMETERSENTRY"]._serialized_start = 230
    _globals["_PARAMETER_PARAMETERSENTRY"]._serialized_end = 291
    _globals["_PARAMETERS"]._serialized_start = 319
    _globals["_PARAMETERS"]._serialized_end = 451
    _globals["_PARAMETERS_ENTRIESENTRY"]._serialized_start = 393
    _globals["_PARAMETERS_ENTRIESENTRY"]._serialized_end = 451
# @@protoc_insertion_point(module_scope)
