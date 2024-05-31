from __future__ import annotations
from dataclasses import dataclass
from pandas import DataFrame, concat, MultiIndex

from .solution_pb2 import State, Solution as _Solution
from .instance_pb2 import Instance as _Instance
from .function_pb2 import Function
from .constraint_pb2 import Equality
from .decision_variables_pb2 import DecisionVariable

from .._ommx_rust import evaluate_instance, used_decision_variable_ids


@dataclass
class Instance:
    raw: _Instance

    @staticmethod
    def from_bytes(data: bytes) -> Instance:
        instance = _Instance()
        instance.ParseFromString(data)
        return Instance(instance)

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    @property
    def decision_variables(self) -> DataFrame:
        return _decision_variables(self.raw)

    @property
    def constraints(self) -> DataFrame:
        constraints = self.raw.constraints
        parameters = DataFrame(dict(v.description.parameters) for v in constraints)
        parameters.columns = MultiIndex.from_product(
            [["parameters"], parameters.columns]
        )
        df = DataFrame(
            {
                "id": c.id,
                "equality": _equality(c.equality),
                "type": _function_type(c.function),
                "used_ids": used_decision_variable_ids(c.function.SerializeToString()),
                "name": c.description.name,
            }
            for c in constraints
        )
        df.columns = MultiIndex.from_product([df.columns, [""]])
        return concat([df, parameters], axis=1).set_index("id")

    def evaluate(self, state: State) -> Solution:
        out, _ = evaluate_instance(self.to_bytes(), state.SerializeToString())
        return Solution.from_bytes(out)


@dataclass
class Solution:
    raw: _Solution

    @staticmethod
    def from_bytes(data: bytes) -> Solution:
        raw = _Solution()
        raw.ParseFromString(data)
        return Solution(raw)

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    @property
    def decision_variables(self) -> DataFrame:
        return _decision_variables(self.raw)

    @property
    def constraints(self) -> DataFrame:
        evaluation = self.raw.evaluated_constraints
        parameters = DataFrame(dict(v.description.parameters) for v in evaluation)
        parameters.columns = MultiIndex.from_product(
            [["parameters"], parameters.columns]
        )
        df = DataFrame(
            {
                "id": v.id,
                "equality": _equality(v.equality),
                "value": v.evaluated_value,
                "used_ids": set(v.used_decision_variable_ids),
                "name": v.description.name,
            }
            for v in evaluation
        )
        df.columns = MultiIndex.from_product([df.columns, [""]])
        return concat([df, parameters], axis=1).set_index("id")


def _decision_variables(obj: _Instance | _Solution) -> DataFrame:
    decision_variables = obj.decision_variables
    parameters = DataFrame(dict(v.description.parameters) for v in decision_variables)
    parameters.columns = MultiIndex.from_product([["parameters"], parameters.columns])
    df = DataFrame(
        {
            "id": v.id,
            "kind": _kind(v.kind),
            "lower": v.bound.lower,
            "upper": v.bound.upper,
            "name": v.description.name,
        }
        for v in decision_variables
    )
    df.columns = MultiIndex.from_product([df.columns, [""]])
    return concat([df, parameters], axis=1).set_index("id")


def _function_type(function: Function) -> str:
    if function.HasField("constant"):
        return "constant"
    if function.HasField("linear"):
        return "linear"
    if function.HasField("quadratic"):
        return "quadratic"
    if function.HasField("polynomial"):
        return "polynomial"
    raise ValueError("Unknown function type")


def _kind(kind: DecisionVariable.Kind.ValueType) -> str:
    if kind == DecisionVariable.Kind.KIND_UNSPECIFIED:
        return "unspecified"
    if kind == DecisionVariable.Kind.KIND_BINARY:
        return "binary"
    if kind == DecisionVariable.Kind.KIND_INTEGER:
        return "integer"
    if kind == DecisionVariable.Kind.KIND_CONTINUOUS:
        return "continuous"
    if kind == DecisionVariable.Kind.KIND_SEMI_INTEGER:
        return "semi-integer"
    if kind == DecisionVariable.Kind.KIND_SEMI_CONTINUOUS:
        return "semi-continuous"
    raise ValueError("Unknown kind")


def _equality(equality: Equality.ValueType) -> str:
    if equality == Equality.EQUALITY_EQUAL_TO_ZERO:
        return "=0"
    if equality == Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO:
        return "<=0"
    raise ValueError("Unknown equality")
