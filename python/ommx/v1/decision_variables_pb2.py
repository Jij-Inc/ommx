# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# source: ommx/v1/decision_variables.proto
# Protobuf Python Version: 5.26.1
"""Generated protocol buffer code."""
from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()




DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(b'\n ommx/v1/decision_variables.proto\x12\x07ommx.v1\"\xc7\x02\n\x10\x44\x65\x63isionVariable\x12\x0e\n\x02id\x18\x01 \x01(\x04R\x02id\x12\x32\n\x04kind\x18\x02 \x01(\x0e\x32\x1e.ommx.v1.DecisionVariable.KindR\x04kind\x12L\n\x0b\x64\x65scription\x18\x03 \x01(\x0b\x32%.ommx.v1.DecisionVariable.DescriptionH\x00R\x0b\x64\x65scription\x88\x01\x01\x1a\x41\n\x0b\x44\x65scription\x12\x12\n\x04name\x18\x01 \x01(\tR\x04name\x12\x1e\n\nsubscripts\x18\x02 \x03(\x04R\nsubscripts\"N\n\x04Kind\x12\x14\n\x10KIND_UNSPECIFIED\x10\x00\x12\x0f\n\x0bKIND_BINARY\x10\x01\x12\x10\n\x0cKIND_INTEGER\x10\x02\x12\r\n\tKIND_REAL\x10\x03\x42\x0e\n\x0c_descriptionBb\n\x0b\x63om.ommx.v1B\x16\x44\x65\x63isionVariablesProtoP\x01\xa2\x02\x03OXX\xaa\x02\x07Ommx.V1\xca\x02\x07Ommx\\V1\xe2\x02\x13Ommx\\V1\\GPBMetadata\xea\x02\x08Ommx::V1b\x06proto3')

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, 'ommx.v1.decision_variables_pb2', _globals)
if not _descriptor._USE_C_DESCRIPTORS:
  _globals['DESCRIPTOR']._loaded_options = None
  _globals['DESCRIPTOR']._serialized_options = b'\n\013com.ommx.v1B\026DecisionVariablesProtoP\001\242\002\003OXX\252\002\007Ommx.V1\312\002\007Ommx\\V1\342\002\023Ommx\\V1\\GPBMetadata\352\002\010Ommx::V1'
  _globals['_DECISIONVARIABLE']._serialized_start=46
  _globals['_DECISIONVARIABLE']._serialized_end=373
  _globals['_DECISIONVARIABLE_DESCRIPTION']._serialized_start=212
  _globals['_DECISIONVARIABLE_DESCRIPTION']._serialized_end=277
  _globals['_DECISIONVARIABLE_KIND']._serialized_start=279
  _globals['_DECISIONVARIABLE_KIND']._serialized_end=357
# @@protoc_insertion_point(module_scope)
