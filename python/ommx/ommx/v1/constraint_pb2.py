# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# NO CHECKED-IN PROTOBUF GENCODE
# source: ommx/v1/constraint.proto
# Protobuf Python Version: 5.28.3
"""Generated protocol buffer code."""

from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import runtime_version as _runtime_version
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder

_runtime_version.ValidateProtobufRuntimeVersion(
    _runtime_version.Domain.PUBLIC, 5, 28, 3, "", "ommx/v1/constraint.proto"
)
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()


from ommx.v1 import function_pb2 as ommx_dot_v1_dot_function__pb2


DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(
    b"\n\x18ommx/v1/constraint.proto\x12\x07ommx.v1\x1a\x16ommx/v1/function.proto\"\xf7\x02\n\nConstraint\x12\x0e\n\x02id\x18\x01 \x01(\x04R\x02id\x12-\n\x08\x65quality\x18\x02 \x01(\x0e\x32\x11.ommx.v1.EqualityR\x08\x65quality\x12-\n\x08\x66unction\x18\x03 \x01(\x0b\x32\x11.ommx.v1.FunctionR\x08\x66unction\x12\x1e\n\nsubscripts\x18\x08 \x03(\x03R\nsubscripts\x12\x43\n\nparameters\x18\x05 \x03(\x0b\x32#.ommx.v1.Constraint.ParametersEntryR\nparameters\x12\x17\n\x04name\x18\x06 \x01(\tH\x00R\x04name\x88\x01\x01\x12%\n\x0b\x64\x65scription\x18\x07 \x01(\tH\x01R\x0b\x64\x65scription\x88\x01\x01\x1a=\n\x0fParametersEntry\x12\x10\n\x03key\x18\x01 \x01(\tR\x03key\x12\x14\n\x05value\x18\x02 \x01(\tR\x05value:\x02\x38\x01\x42\x07\n\x05_nameB\x0e\n\x0c_description\"\xfc\x03\n\x13\x45valuatedConstraint\x12\x0e\n\x02id\x18\x01 \x01(\x04R\x02id\x12-\n\x08\x65quality\x18\x02 \x01(\x0e\x32\x11.ommx.v1.EqualityR\x08\x65quality\x12'\n\x0f\x65valuated_value\x18\x03 \x01(\x01R\x0e\x65valuatedValue\x12;\n\x1aused_decision_variable_ids\x18\x04 \x03(\x04R\x17usedDecisionVariableIds\x12\x1e\n\nsubscripts\x18\t \x03(\x03R\nsubscripts\x12L\n\nparameters\x18\x05 \x03(\x0b\x32,.ommx.v1.EvaluatedConstraint.ParametersEntryR\nparameters\x12\x17\n\x04name\x18\x06 \x01(\tH\x00R\x04name\x88\x01\x01\x12%\n\x0b\x64\x65scription\x18\x07 \x01(\tH\x01R\x0b\x64\x65scription\x88\x01\x01\x12(\n\rdual_variable\x18\x08 \x01(\x01H\x02R\x0c\x64ualVariable\x88\x01\x01\x1a=\n\x0fParametersEntry\x12\x10\n\x03key\x18\x01 \x01(\tR\x03key\x12\x14\n\x05value\x18\x02 \x01(\tR\x05value:\x02\x38\x01\x42\x07\n\x05_nameB\x0e\n\x0c_descriptionB\x10\n\x0e_dual_variable*i\n\x08\x45quality\x12\x18\n\x14\x45QUALITY_UNSPECIFIED\x10\x00\x12\x1a\n\x16\x45QUALITY_EQUAL_TO_ZERO\x10\x01\x12'\n#EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO\x10\x02\x42[\n\x0b\x63om.ommx.v1B\x0f\x43onstraintProtoP\x01\xa2\x02\x03OXX\xaa\x02\x07Ommx.V1\xca\x02\x07Ommx\\V1\xe2\x02\x13Ommx\\V1\\GPBMetadata\xea\x02\x08Ommx::V1b\x06proto3"
)

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, "ommx.v1.constraint_pb2", _globals)
if not _descriptor._USE_C_DESCRIPTORS:
    _globals["DESCRIPTOR"]._loaded_options = None
    _globals[
        "DESCRIPTOR"
    ]._serialized_options = b"\n\013com.ommx.v1B\017ConstraintProtoP\001\242\002\003OXX\252\002\007Ommx.V1\312\002\007Ommx\\V1\342\002\023Ommx\\V1\\GPBMetadata\352\002\010Ommx::V1"
    _globals["_CONSTRAINT_PARAMETERSENTRY"]._loaded_options = None
    _globals["_CONSTRAINT_PARAMETERSENTRY"]._serialized_options = b"8\001"
    _globals["_EVALUATEDCONSTRAINT_PARAMETERSENTRY"]._loaded_options = None
    _globals["_EVALUATEDCONSTRAINT_PARAMETERSENTRY"]._serialized_options = b"8\001"
    _globals["_EQUALITY"]._serialized_start = 950
    _globals["_EQUALITY"]._serialized_end = 1055
    _globals["_CONSTRAINT"]._serialized_start = 62
    _globals["_CONSTRAINT"]._serialized_end = 437
    _globals["_CONSTRAINT_PARAMETERSENTRY"]._serialized_start = 351
    _globals["_CONSTRAINT_PARAMETERSENTRY"]._serialized_end = 412
    _globals["_EVALUATEDCONSTRAINT"]._serialized_start = 440
    _globals["_EVALUATEDCONSTRAINT"]._serialized_end = 948
    _globals["_EVALUATEDCONSTRAINT_PARAMETERSENTRY"]._serialized_start = 351
    _globals["_EVALUATEDCONSTRAINT_PARAMETERSENTRY"]._serialized_end = 412
# @@protoc_insertion_point(module_scope)
