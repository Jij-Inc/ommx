import enum

import numpy as np

from ommx.v1 import Instance, DecisionVariable
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1.solution_pb2 import State


class DataType(enum.Enum):
    INT = enum.auto()
    FLOAT = enum.auto()


class SingleFeasibleLPGenerator:
    INT_LOWER_BOUND = -100
    INT_UPPER_BOUND = 100
    FLOAT_LOWER_BOUND = -100.0
    FLOAT_UPPER_BOUND = 100.0

    def __init__(self, n: int, data_type: DataType):
        """
        The class generates a test instance as follows:

        Objective function: 0
        Constraints: A @ x = b    (A: regular matrix, b: constant vector)

        So the generated instance has a unique solution `x`.

        Args:
            n (int): The size of the matrix and the vectors.
            data_type (DataType): The data type of the matrix and the vectors.

        Raises:
            ValueError: If `n` is not a positive integer or `data_type` is not DataType.
        """
        if n <= 0:
            raise ValueError("`n` must be a positive integer.")
        if data_type not in DataType:
            raise ValueError("`data_type` must be DataType.")

        self._A = self._generate_random_reguler_matrix(n, data_type)
        self._x = self._generate_random_solution(n, data_type)
        self._b = self._A @ self._x
        self._data_type = data_type

    def _generate_random_reguler_matrix(
        self,
        n: int,
        data_type: DataType,
    ) -> np.ndarray:
        while True:
            if data_type == DataType.INT:
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
        data_type: DataType,
    ) -> np.ndarray:
        if data_type == DataType.INT:
            return np.random.randint(
                low=self.INT_LOWER_BOUND,
                high=self.INT_UPPER_BOUND,
                size=n,
            )
        else:
            return np.random.uniform(
                low=self.FLOAT_LOWER_BOUND, high=self.FLOAT_UPPER_BOUND, size=n
            )

    def get_v1_instance(self) -> Instance:
        """
        Get an instance of a linear programming problem with a unique solution.

        Examples:
            >>> from ommx.testing import DataType, SingleFeasibleLPGenerator
            >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
            >>> ommx_instance = generator.get_v1_instance()
        """
        # define decision variables
        if self._data_type == DataType.INT:
            decision_variables = [
                DecisionVariable.integer(
                    i,
                    lower=self.INT_LOWER_BOUND,
                    upper=self.INT_UPPER_BOUND,
                )
                for i in range(len(self._x))
            ]
        else:
            decision_variables = [
                DecisionVariable.continuous(
                    i,
                    lower=self.FLOAT_LOWER_BOUND,
                    upper=self.FLOAT_UPPER_BOUND,
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
                equality=Equality.EQUALITY_EQUAL_TO_ZERO,
                function=Function(constant=-self._b[i], linear=linear),
            )
            constraints.append(constraint)

        return Instance.from_components(
            description=Instance.Description(name="LPTest"),
            decision_variables=decision_variables,
            objective=Function(constant=0),
            constraints=constraints,
            sense=Instance.MINIMIZE,
        )

    def get_v1_state(self) -> State:
        """
        Get the solution state of the generated instance.

        Examples:
            >>> from ommx.testing import DataType, SingleFeasibleLPGenerator
            >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
            >>> ommx_state = generator.get_v1_state()
        """
        return State(entries={i: value for i, value in enumerate(self._x)})
