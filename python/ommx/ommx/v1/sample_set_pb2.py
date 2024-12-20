# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# source: ommx/v1/sample_set.proto
# Protobuf Python Version: 5.26.1
"""Generated protocol buffer code."""

from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()


from ommx.v1 import constraint_pb2 as ommx_dot_v1_dot_constraint__pb2
from ommx.v1 import decision_variables_pb2 as ommx_dot_v1_dot_decision__variables__pb2
from ommx.v1 import solution_pb2 as ommx_dot_v1_dot_solution__pb2


DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(
    b'\n\x18ommx/v1/sample_set.proto\x12\x07ommx.v1\x1a\x18ommx/v1/constraint.proto\x1a ommx/v1/decision_variables.proto\x1a\x16ommx/v1/solution.proto"|\n\x07Samples\x12\x30\n\x07\x65ntries\x18\x01 \x03(\x0b\x32\x16.ommx.v1.Samples.EntryR\x07\x65ntries\x1a?\n\x05\x45ntry\x12$\n\x05state\x18\x01 \x01(\x0b\x32\x0e.ommx.v1.StateR\x05state\x12\x10\n\x03ids\x18\x02 \x03(\x04R\x03ids"x\n\rSampledValues\x12\x36\n\x07\x65ntries\x18\x01 \x03(\x0b\x32\x1c.ommx.v1.SampledValues.EntryR\x07\x65ntries\x1a/\n\x05\x45ntry\x12\x14\n\x05value\x18\x01 \x01(\x01R\x05value\x12\x10\n\x03ids\x18\x02 \x03(\x04R\x03ids"\xa4\x01\n\x17SampledDecisionVariable\x12\x46\n\x11\x64\x65\x63ision_variable\x18\x01 \x01(\x0b\x32\x19.ommx.v1.DecisionVariableR\x10\x64\x65\x63isionVariable\x12\x35\n\x07samples\x18\x02 \x01(\x0b\x32\x16.ommx.v1.SampledValuesH\x00R\x07samples\x88\x01\x01\x42\n\n\x08_samples"\xd6\x05\n\x11SampledConstraint\x12\x0e\n\x02id\x18\x01 \x01(\x04R\x02id\x12-\n\x08\x65quality\x18\x02 \x01(\x0e\x32\x11.ommx.v1.EqualityR\x08\x65quality\x12\x17\n\x04name\x18\x03 \x01(\tH\x00R\x04name\x88\x01\x01\x12\x1e\n\nsubscripts\x18\x04 \x03(\x03R\nsubscripts\x12J\n\nparameters\x18\x05 \x03(\x0b\x32*.ommx.v1.SampledConstraint.ParametersEntryR\nparameters\x12%\n\x0b\x64\x65scription\x18\x06 \x01(\tH\x01R\x0b\x64\x65scription\x88\x01\x01\x12*\n\x0eremoved_reason\x18\x07 \x01(\tH\x02R\rremovedReason\x88\x01\x01\x12s\n\x19removed_reason_parameters\x18\x08 \x03(\x0b\x32\x37.ommx.v1.SampledConstraint.RemovedReasonParametersEntryR\x17removedReasonParameters\x12\x41\n\x10\x65valuated_values\x18\t \x01(\x0b\x32\x16.ommx.v1.SampledValuesR\x0f\x65valuatedValues\x12;\n\x1aused_decision_variable_ids\x18\n \x03(\x04R\x17usedDecisionVariableIds\x1a=\n\x0fParametersEntry\x12\x10\n\x03key\x18\x01 \x01(\tR\x03key\x12\x14\n\x05value\x18\x02 \x01(\tR\x05value:\x02\x38\x01\x1aJ\n\x1cRemovedReasonParametersEntry\x12\x10\n\x03key\x18\x01 \x01(\tR\x03key\x12\x14\n\x05value\x18\x02 \x01(\tR\x05value:\x02\x38\x01\x42\x07\n\x05_nameB\x0e\n\x0c_descriptionB\x11\n\x0f_removed_reason"\xcd\x02\n\tSampleSet\x12\x36\n\nobjectives\x18\x01 \x01(\x0b\x32\x16.ommx.v1.SampledValuesR\nobjectives\x12O\n\x12\x64\x65\x63ision_variables\x18\x02 \x03(\x0b\x32 .ommx.v1.SampledDecisionVariableR\x11\x64\x65\x63isionVariables\x12<\n\x0b\x63onstraints\x18\x03 \x03(\x0b\x32\x1a.ommx.v1.SampledConstraintR\x0b\x63onstraints\x12<\n\x08\x66\x65\x61sible\x18\x04 \x03(\x0b\x32 .ommx.v1.SampleSet.FeasibleEntryR\x08\x66\x65\x61sible\x1a;\n\rFeasibleEntry\x12\x10\n\x03key\x18\x01 \x01(\x04R\x03key\x12\x14\n\x05value\x18\x02 \x01(\x08R\x05value:\x02\x38\x01\x42Z\n\x0b\x63om.ommx.v1B\x0eSampleSetProtoP\x01\xa2\x02\x03OXX\xaa\x02\x07Ommx.V1\xca\x02\x07Ommx\\V1\xe2\x02\x13Ommx\\V1\\GPBMetadata\xea\x02\x08Ommx::V1b\x06proto3'
)

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, "ommx.v1.sample_set_pb2", _globals)
if not _descriptor._USE_C_DESCRIPTORS:
    _globals["DESCRIPTOR"]._loaded_options = None
    _globals[
        "DESCRIPTOR"
    ]._serialized_options = b"\n\013com.ommx.v1B\016SampleSetProtoP\001\242\002\003OXX\252\002\007Ommx.V1\312\002\007Ommx\\V1\342\002\023Ommx\\V1\\GPBMetadata\352\002\010Ommx::V1"
    _globals["_SAMPLEDCONSTRAINT_PARAMETERSENTRY"]._loaded_options = None
    _globals["_SAMPLEDCONSTRAINT_PARAMETERSENTRY"]._serialized_options = b"8\001"
    _globals["_SAMPLEDCONSTRAINT_REMOVEDREASONPARAMETERSENTRY"]._loaded_options = None
    _globals[
        "_SAMPLEDCONSTRAINT_REMOVEDREASONPARAMETERSENTRY"
    ]._serialized_options = b"8\001"
    _globals["_SAMPLESET_FEASIBLEENTRY"]._loaded_options = None
    _globals["_SAMPLESET_FEASIBLEENTRY"]._serialized_options = b"8\001"
    _globals["_SAMPLES"]._serialized_start = 121
    _globals["_SAMPLES"]._serialized_end = 245
    _globals["_SAMPLES_ENTRY"]._serialized_start = 182
    _globals["_SAMPLES_ENTRY"]._serialized_end = 245
    _globals["_SAMPLEDVALUES"]._serialized_start = 247
    _globals["_SAMPLEDVALUES"]._serialized_end = 367
    _globals["_SAMPLEDVALUES_ENTRY"]._serialized_start = 320
    _globals["_SAMPLEDVALUES_ENTRY"]._serialized_end = 367
    _globals["_SAMPLEDDECISIONVARIABLE"]._serialized_start = 370
    _globals["_SAMPLEDDECISIONVARIABLE"]._serialized_end = 534
    _globals["_SAMPLEDCONSTRAINT"]._serialized_start = 537
    _globals["_SAMPLEDCONSTRAINT"]._serialized_end = 1263
    _globals["_SAMPLEDCONSTRAINT_PARAMETERSENTRY"]._serialized_start = 1082
    _globals["_SAMPLEDCONSTRAINT_PARAMETERSENTRY"]._serialized_end = 1143
    _globals["_SAMPLEDCONSTRAINT_REMOVEDREASONPARAMETERSENTRY"]._serialized_start = 1145
    _globals["_SAMPLEDCONSTRAINT_REMOVEDREASONPARAMETERSENTRY"]._serialized_end = 1219
    _globals["_SAMPLESET"]._serialized_start = 1266
    _globals["_SAMPLESET"]._serialized_end = 1599
    _globals["_SAMPLESET_FEASIBLEENTRY"]._serialized_start = 1540
    _globals["_SAMPLESET_FEASIBLEENTRY"]._serialized_end = 1599
# @@protoc_insertion_point(module_scope)
