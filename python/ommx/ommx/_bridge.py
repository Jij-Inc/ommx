"""Private factories for ProtobufV1 transfers into the Python SDK 2.x model."""

from collections.abc import Callable
from typing import TypeVar

from . import _ommx_rust
from ._bridge_error import BridgeError
from .v1 import (
    Constraint,
    DecisionVariable,
    Function,
    Instance,
    ParametricInstance,
    SampleSet,
    Solution,
)
from .v1.parametric_instance_pb2 import ParametricInstance as RawParametricInstance

_T = TypeVar("_T")


def _parse(parser: Callable[[bytes], _T], data: bytes) -> _T:
    try:
        return parser(data)
    except Exception as error:
        raise BridgeError(f"Invalid ProtobufV1 bridge payload: {error}") from error


def function(data: bytes) -> Function:
    return Function.from_raw(_parse(_ommx_rust.Function.from_bytes, data))


def constraint(data: bytes) -> Constraint:
    # from_raw preserves the incoming ID and advances the legacy ID allocator.
    return Constraint.from_raw(_parse(_ommx_rust.Constraint.from_bytes, data))


def decision_variable(data: bytes) -> DecisionVariable:
    return DecisionVariable(_parse(_ommx_rust.DecisionVariable.from_bytes, data))


def instance(data: bytes) -> Instance:
    return Instance(_parse(_ommx_rust.Instance.from_bytes, data))


def parametric_instance(data: bytes) -> ParametricInstance:
    # The public legacy constructor only decodes protobuf. Validate every
    # function and reference with the SDK first, including unknown oneof arms.
    _parse(_ommx_rust.ParametricInstance.from_bytes, data)
    raw = RawParametricInstance()
    raw.ParseFromString(data)
    return ParametricInstance(raw)


def solution(data: bytes) -> Solution:
    return Solution(_parse(_ommx_rust.Solution.from_bytes, data))


def sample_set(data: bytes) -> SampleSet:
    return SampleSet(_parse(_ommx_rust.SampleSet.from_bytes, data))
