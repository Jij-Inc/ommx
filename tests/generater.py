import enum

import numpy as np

from ommx.v1.constraint_pb2 import Constraint
from ommx.v1.decision_variables_pb2 import DecisionVariable, Bound
from ommx.v1.function_pb2 import Function
from ommx.v1.instance_pb2 import Instance
from ommx.v1.linear_pb2 import Linear
from ommx.v1.solution_pb2 import Solution, SolutionList


class LPTestDataType(enum.Enum):
    INT = enum.auto()
    FLOAT = enum.auto()


class LPTestGenerater:
    """
    The class generates a test instance as follows:

    Objective function: 0
    Constraints: A @ x = b    (A: regular matrix, b: constant vector)

    So the generated instance has a unique solution `x`.
    """
    INT_LOWER_BOUND = -100
    INT_UPPER_BOUND = 100
    FLOAT_LOWER_BOUND = -100.0
    FLOAT_UPPER_BOUND = 100.0

    def __init__(self, n: int, data_type: LPTestDataType):
        if n <= 0:
            raise ValueError("`n` must be a positive integer.")
        if data_type not in LPTestDataType:
            raise ValueError("`data_type` must be LPTestDataType.")
        
        self._A = self._generate_random_reguler_matrix(n, data_type)
        self._x = self._generate_random_solution(n, data_type)
        self._b = self._A @ self._x
        self._data_type = data_type

        
    def _generate_random_reguler_matrix(
        self,
        n: int,
        data_type: LPTestDataType,
    ) -> np.ndarray:
        while True:
            if data_type == LPTestDataType.INT:
                matrix = np.random.randint(
                    low=self.INT_LOWER_BOUND,
                    high=self.INT_UPPER_BOUND,
                    size=(n, n),
                )
            else:
                matrix = np.random.rand(n, n)

            if np.linalg.det(matrix) != 0:
                return matrix

            
    def _generate_random_solution(
        self,
        n: int,
        data_type: LPTestDataType,
    ) -> np.ndarray:
        if data_type == LPTestDataType.INT:
            return np.random.randint(
                low=self.INT_LOWER_BOUND,
                high=self.INT_UPPER_BOUND,
                size=n,
            )
        else:
            return np.random.uniform(
                low=self.FLOAT_LOWER_BOUND,
                high=self.FLOAT_UPPER_BOUND,
                size=n
            )


    def get_instance(self) -> bytes:
        # define decision variables
        if self._data_type == LPTestDataType.INT:
            decision_variables = [
                DecisionVariable(
                    id=i,
                    kind=DecisionVariable.Kind.KIND_INTEGER,
                    bound=Bound(
                        lower=self.INT_LOWER_BOUND,
                        upper=self.INT_UPPER_BOUND,
                    )
                )
                for i in range(len(self._x))
            ]
        else:
            decision_variables = [
                DecisionVariable(
                    id=i,
                    kind=DecisionVariable.Kind.KIND_CONTINUOUS,
                    bound=Bound(
                        lower= self.FLOAT_LOWER_BOUND,
                        upper= self.FLOAT_UPPER_BOUND,
                    )
                )
                for i in range(len(self._x))
            ]

        # define constraints
        constraints = []
        for i in range(len(self._b)):
            linear = Linear(
                terms=[
                    Linear.Term(id=j, coefficient=value)
                    for j, value in enumerate(self._A[i])
                ],
                constant=-self._b[i],
            )
            
            constraint = Constraint(
                id=i,
                equality=Constraint.Equality.EQUALITY_EQUAL_TO_ZERO,
                function=Function(constant=-self._b[i], linear=linear),
            )
            constraints.append(constraint)

        return Instance(
            description=Instance.Description(name="LPTest"),
            decision_variables=decision_variables,
            objective=Function(constant=0),
            constraints=constraints,
        ).SerializeToString()


    def get_solution(self) -> bytes:
        solution = Solution(
            entries={i: value for i, value in enumerate(self._x)}
        )
        return SolutionList(solutions=[solution]).SerializeToString()
