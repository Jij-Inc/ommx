# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# source: ommx/v1/quadratic.proto
# Protobuf Python Version: 5.26.1
"""Generated protocol buffer code."""
from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()


from ommx.v1 import linear_pb2 as ommx_dot_v1_dot_linear__pb2
from ommx.v1 import sparse_matrix_pb2 as ommx_dot_v1_dot_sparse__matrix__pb2


DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(b'\n\x17ommx/v1/quadratic.proto\x12\x07ommx.v1\x1a\x14ommx/v1/linear.proto\x1a\x1bommx/v1/sparse_matrix.proto\"y\n\tQuadratic\x12\x33\n\tquadradic\x18\x01 \x01(\x0b\x32\x15.ommx.v1.SparseMatrixR\tquadradic\x12,\n\x06linear\x18\x02 \x01(\x0b\x32\x0f.ommx.v1.LinearH\x00R\x06linear\x88\x01\x01\x42\t\n\x07_linearBZ\n\x0b\x63om.ommx.v1B\x0eQuadraticProtoP\x01\xa2\x02\x03OXX\xaa\x02\x07Ommx.V1\xca\x02\x07Ommx\\V1\xe2\x02\x13Ommx\\V1\\GPBMetadata\xea\x02\x08Ommx::V1b\x06proto3')

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, 'ommx.v1.quadratic_pb2', _globals)
if not _descriptor._USE_C_DESCRIPTORS:
  _globals['DESCRIPTOR']._loaded_options = None
  _globals['DESCRIPTOR']._serialized_options = b'\n\013com.ommx.v1B\016QuadraticProtoP\001\242\002\003OXX\252\002\007Ommx.V1\312\002\007Ommx\\V1\342\002\023Ommx\\V1\\GPBMetadata\352\002\010Ommx::V1'
  _globals['_QUADRATIC']._serialized_start=87
  _globals['_QUADRATIC']._serialized_end=208
# @@protoc_insertion_point(module_scope)