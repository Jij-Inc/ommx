# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# NO CHECKED-IN PROTOBUF GENCODE
# source: ommx/v1/polynomial.proto
# Protobuf Python Version: 6.30.2
"""Generated protocol buffer code."""

from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import runtime_version as _runtime_version
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder

_runtime_version.ValidateProtobufRuntimeVersion(
    _runtime_version.Domain.PUBLIC, 6, 30, 2, "", "ommx/v1/polynomial.proto"
)
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()


DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(
    b'\n\x18ommx/v1/polynomial.proto\x12\x07ommx.v1">\n\x08Monomial\x12\x10\n\x03ids\x18\x01 \x03(\x04R\x03ids\x12 \n\x0b\x63oefficient\x18\x02 \x01(\x01R\x0b\x63oefficient"5\n\nPolynomial\x12\'\n\x05terms\x18\x01 \x03(\x0b\x32\x11.ommx.v1.MonomialR\x05termsb\x06proto3'
)

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, "ommx.v1.polynomial_pb2", _globals)
if not _descriptor._USE_C_DESCRIPTORS:
    DESCRIPTOR._loaded_options = None
    _globals["_MONOMIAL"]._serialized_start = 37
    _globals["_MONOMIAL"]._serialized_end = 99
    _globals["_POLYNOMIAL"]._serialized_start = 101
    _globals["_POLYNOMIAL"]._serialized_end = 154
# @@protoc_insertion_point(module_scope)
