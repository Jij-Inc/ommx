# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# NO CHECKED-IN PROTOBUF GENCODE
# source: ommx/v1/k_hot.proto
# Protobuf Python Version: 6.30.2
"""Generated protocol buffer code."""

from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import runtime_version as _runtime_version
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder

_runtime_version.ValidateProtobufRuntimeVersion(
    _runtime_version.Domain.PUBLIC, 6, 30, 2, "", "ommx/v1/k_hot.proto"
)
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()


DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(
    b'\n\x13ommx/v1/k_hot.proto\x12\x07ommx.v1"|\n\x04KHot\x12#\n\rconstraint_id\x18\x01 \x01(\x04R\x0c\x63onstraintId\x12-\n\x12\x64\x65\x63ision_variables\x18\x02 \x03(\x04R\x11\x64\x65\x63isionVariables\x12 \n\x0cnum_hot_vars\x18\x03 \x01(\x04R\nnumHotVarsb\x06proto3'
)

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, "ommx.v1.k_hot_pb2", _globals)
if not _descriptor._USE_C_DESCRIPTORS:
    DESCRIPTOR._loaded_options = None
    _globals["_KHOT"]._serialized_start = 32
    _globals["_KHOT"]._serialized_end = 156
# @@protoc_insertion_point(module_scope)
