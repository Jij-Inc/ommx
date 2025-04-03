# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# NO CHECKED-IN PROTOBUF GENCODE
# source: ommx/v1/instance.proto
# Protobuf Python Version: 6.30.2
"""Generated protocol buffer code."""

from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import runtime_version as _runtime_version
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder

_runtime_version.ValidateProtobufRuntimeVersion(
    _runtime_version.Domain.PUBLIC, 6, 30, 2, "", "ommx/v1/instance.proto"
)
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()


from ommx.v1 import constraint_pb2 as ommx_dot_v1_dot_constraint__pb2
from ommx.v1 import constraint_hints_pb2 as ommx_dot_v1_dot_constraint__hints__pb2
from ommx.v1 import decision_variables_pb2 as ommx_dot_v1_dot_decision__variables__pb2
from ommx.v1 import function_pb2 as ommx_dot_v1_dot_function__pb2


DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(
    b'\n\x16ommx/v1/instance.proto\x12\x07ommx.v1\x1a\x18ommx/v1/constraint.proto\x1a\x1eommx/v1/constraint_hints.proto\x1a ommx/v1/decision_variables.proto\x1a\x16ommx/v1/function.proto"\x84\x01\n\nParameters\x12:\n\x07\x65ntries\x18\x01 \x03(\x0b\x32 .ommx.v1.Parameters.EntriesEntryR\x07\x65ntries\x1a:\n\x0c\x45ntriesEntry\x12\x10\n\x03key\x18\x01 \x01(\x04R\x03key\x12\x14\n\x05value\x18\x02 \x01(\x01R\x05value:\x02\x38\x01"\xdc\x07\n\x08Instance\x12?\n\x0b\x64\x65scription\x18\x01 \x01(\x0b\x32\x1d.ommx.v1.Instance.DescriptionR\x0b\x64\x65scription\x12H\n\x12\x64\x65\x63ision_variables\x18\x02 \x03(\x0b\x32\x19.ommx.v1.DecisionVariableR\x11\x64\x65\x63isionVariables\x12/\n\tobjective\x18\x03 \x01(\x0b\x32\x11.ommx.v1.FunctionR\tobjective\x12\x35\n\x0b\x63onstraints\x18\x04 \x03(\x0b\x32\x13.ommx.v1.ConstraintR\x0b\x63onstraints\x12-\n\x05sense\x18\x05 \x01(\x0e\x32\x17.ommx.v1.Instance.SenseR\x05sense\x12\x38\n\nparameters\x18\x06 \x01(\x0b\x32\x13.ommx.v1.ParametersH\x00R\nparameters\x88\x01\x01\x12\x43\n\x10\x63onstraint_hints\x18\x07 \x01(\x0b\x32\x18.ommx.v1.ConstraintHintsR\x0f\x63onstraintHints\x12K\n\x13removed_constraints\x18\x08 \x03(\x0b\x32\x1a.ommx.v1.RemovedConstraintR\x12removedConstraints\x12s\n\x1c\x64\x65\x63ision_variable_dependency\x18\t \x03(\x0b\x32\x31.ommx.v1.Instance.DecisionVariableDependencyEntryR\x1a\x64\x65\x63isionVariableDependency\x1a\xb3\x01\n\x0b\x44\x65scription\x12\x17\n\x04name\x18\x01 \x01(\tH\x00R\x04name\x88\x01\x01\x12%\n\x0b\x64\x65scription\x18\x02 \x01(\tH\x01R\x0b\x64\x65scription\x88\x01\x01\x12\x18\n\x07\x61uthors\x18\x03 \x03(\tR\x07\x61uthors\x12"\n\ncreated_by\x18\x04 \x01(\tH\x02R\tcreatedBy\x88\x01\x01\x42\x07\n\x05_nameB\x0e\n\x0c_descriptionB\r\n\x0b_created_by\x1a`\n\x1f\x44\x65\x63isionVariableDependencyEntry\x12\x10\n\x03key\x18\x01 \x01(\x04R\x03key\x12\'\n\x05value\x18\x02 \x01(\x0b\x32\x11.ommx.v1.FunctionR\x05value:\x02\x38\x01"F\n\x05Sense\x12\x15\n\x11SENSE_UNSPECIFIED\x10\x00\x12\x12\n\x0eSENSE_MINIMIZE\x10\x01\x12\x12\n\x0eSENSE_MAXIMIZE\x10\x02\x42\r\n\x0b_parametersb\x06proto3'
)

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, "ommx.v1.instance_pb2", _globals)
if not _descriptor._USE_C_DESCRIPTORS:
    DESCRIPTOR._loaded_options = None
    _globals["_PARAMETERS_ENTRIESENTRY"]._loaded_options = None
    _globals["_PARAMETERS_ENTRIESENTRY"]._serialized_options = b"8\001"
    _globals["_INSTANCE_DECISIONVARIABLEDEPENDENCYENTRY"]._loaded_options = None
    _globals["_INSTANCE_DECISIONVARIABLEDEPENDENCYENTRY"]._serialized_options = b"8\001"
    _globals["_PARAMETERS"]._serialized_start = 152
    _globals["_PARAMETERS"]._serialized_end = 284
    _globals["_PARAMETERS_ENTRIESENTRY"]._serialized_start = 226
    _globals["_PARAMETERS_ENTRIESENTRY"]._serialized_end = 284
    _globals["_INSTANCE"]._serialized_start = 287
    _globals["_INSTANCE"]._serialized_end = 1275
    _globals["_INSTANCE_DESCRIPTION"]._serialized_start = 911
    _globals["_INSTANCE_DESCRIPTION"]._serialized_end = 1090
    _globals["_INSTANCE_DECISIONVARIABLEDEPENDENCYENTRY"]._serialized_start = 1092
    _globals["_INSTANCE_DECISIONVARIABLEDEPENDENCYENTRY"]._serialized_end = 1188
    _globals["_INSTANCE_SENSE"]._serialized_start = 1190
    _globals["_INSTANCE_SENSE"]._serialized_end = 1260
# @@protoc_insertion_point(module_scope)
