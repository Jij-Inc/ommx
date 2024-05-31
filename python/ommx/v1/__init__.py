from __future__ import annotations
from dataclasses import dataclass
from pandas import DataFrame, concat, MultiIndex

from .solution_pb2 import State, Solution as _Solution
from .instance_pb2 import Instance as _Instance

from .._ommx_rust import evaluate_instance


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
        raise NotImplementedError()

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
                "equality": v.equality,
                "value": v.evaluated_value,
                "used_ids": v.used_decision_variable_ids,
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
            "kind": v.kind,
            "lower": v.bound.lower,
            "upper": v.bound.upper,
            "name": v.description.name,
        }
        for v in decision_variables
    )
    df.columns = MultiIndex.from_product([df.columns, [""]])
    return concat([df, parameters], axis=1).set_index("id")
