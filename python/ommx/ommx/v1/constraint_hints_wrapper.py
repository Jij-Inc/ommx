from collections import defaultdict
from typing import Dict, List, Set

from .constraint_hints_pb2 import ConstraintHints as ProtoConstraintHints
from .k_hot_pb2 import KHot
from .one_hot_pb2 import OneHot


class ConstraintHintsWrapper:
    """Wrapper for ConstraintHints to provide additional methods."""

    def __init__(self, constraint_hints: ProtoConstraintHints):
        self._proto = constraint_hints

    @property
    def proto(self) -> ProtoConstraintHints:
        """Returns the underlying protobuf message."""
        return self._proto

    def one_hot_constraints(self) -> List[OneHot]:
        """
        Returns all one-hot constraints, including both deprecated one_hot_constraints field
        and k_hot_constraints[1] field, removing duplicated constraint IDs.
        """
        result = list(self._proto.one_hot_constraints)
        constraint_ids = {c.constraint_id for c in result}

        if 1 in self._proto.k_hot_constraints:
            k_hot_list = self._proto.k_hot_constraints[1]
            for k_hot in k_hot_list.constraints:
                if k_hot.constraint_id not in constraint_ids:
                    constraint_ids.add(k_hot.constraint_id)
                    result.append(
                        OneHot(
                            constraint_id=k_hot.constraint_id,
                            decision_variables=k_hot.decision_variables,
                        )
                    )

        return result

    def k_hot_constraints(self) -> Dict[int, List[KHot]]:
        """
        Returns all k-hot constraints, including both deprecated one_hot_constraints field (as k=1)
        and k_hot_constraints field, removing duplicated constraint IDs.
        """
        result = defaultdict(list)

        for k, k_hot_list in self._proto.k_hot_constraints.items():
            result[k].extend(k_hot_list.constraints)

        k1_constraint_ids = {c.constraint_id for c in result[1]}

        for one_hot in self._proto.one_hot_constraints:
            if one_hot.constraint_id not in k1_constraint_ids:
                k1_constraint_ids.add(one_hot.constraint_id)
                result[1].append(
                    KHot(
                        constraint_id=one_hot.constraint_id,
                        decision_variables=one_hot.decision_variables,
                        num_hot_vars=1,
                    )
                )

        return dict(result)
