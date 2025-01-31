from __future__ import annotations
from dataclasses import dataclass
from typing import final
import mip

from mip.exceptions import ParameterNotAvailable
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1 import Instance, DecisionVariable

from .exception import OMMXPythonMIPAdapterError


@dataclass
class OMMXInstanceBuilder:
    """
    Build ommx.v1.Instance from Python-MIP Model.
    """

    model: mip.Model

    def decision_variables(self) -> list[DecisionVariable]:
        """
        Gather decision variables from Python-MIP Model as ommx.v1.DecisionVariable.
        """
        decision_variables = []
        for var in self.model.vars:
            if var.var_type == mip.BINARY:
                kind = DecisionVariable.BINARY
            elif var.var_type == mip.INTEGER:
                kind = DecisionVariable.INTEGER
            elif var.var_type == mip.CONTINUOUS:
                kind = DecisionVariable.CONTINUOUS
            else:
                raise OMMXPythonMIPAdapterError(
                    f"Not supported variable type. "
                    f"idx: {var.idx} name: {var.name}, type: {var.var_type}"
                )
            decision_variables.append(
                DecisionVariable.of_type(
                    kind, var.idx, lower=var.lb, upper=var.ub, name=var.name
                )
            )
        return decision_variables

    def as_ommx_function(self, lin_expr: mip.LinExpr) -> Function:
        terms = [
            Linear.Term(id=var.idx, coefficient=coefficient)  # type: ignore
            for var, coefficient in lin_expr.expr.items()
        ]
        constant: float = lin_expr.const  # type: ignore

        # If the terms are empty, the function is a constant.
        if len(terms) == 0:
            return Function(constant=constant)
        else:
            return Function(linear=Linear(terms=terms, constant=constant))

    def objective(self) -> Function:
        # In Python-MIP, it is allowed not to set the objective function.
        # If it isn't set, the model behaves as if the objective function is set to 0.
        # However, an error occurs when accessing `.objective`.
        # So if an error occurs, treat the objective function as 0.
        try:
            objective = self.model.objective
        except ParameterNotAvailable:
            return Function(constant=0)

        return self.as_ommx_function(objective)

    def constraints(self) -> list[Constraint]:
        constraints = []

        for constr in self.model.constrs:
            id = constr.idx
            lin_expr = constr.expr
            name = constr.name

            if lin_expr.sense == "=":
                constraint = Constraint(
                    id=id,
                    equality=Equality.EQUALITY_EQUAL_TO_ZERO,
                    function=self.as_ommx_function(lin_expr),
                    name=name,
                )
            elif lin_expr.sense == "<":
                constraint = Constraint(
                    id=id,
                    equality=Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO,
                    function=self.as_ommx_function(lin_expr),
                    name=name,
                )
            elif lin_expr.sense == ">":
                # `ommx.v1.Constraint` does not support `GREATER_THAN_OR_EQUAL_TO_ZERO`.
                # So multiply the linear expression by -1.
                constraint = Constraint(
                    id=id,
                    equality=Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO,
                    function=self.as_ommx_function(-lin_expr),
                    name=name,
                )
            else:
                raise OMMXPythonMIPAdapterError(
                    f"Not supported constraint sense: "
                    f"name: {constr.name}, sense: {lin_expr.sense}"
                )

            constraints.append(constraint)

        return constraints

    def sense(self):
        if self.model.sense == mip.MAXIMIZE:
            return Instance.MAXIMIZE
        elif self.model.sense == mip.MINIMIZE:
            return Instance.MINIMIZE
        raise OMMXPythonMIPAdapterError(f"Not supported sense: {self.model.sense}")

    @final
    def build(self) -> Instance:
        return Instance.from_components(
            decision_variables=self.decision_variables(),
            objective=self.objective(),
            constraints=self.constraints(),
            sense=self.sense(),
        )


def model_to_instance(model: mip.Model) -> Instance:
    """
    The function to convert Python-MIP Model to ommx.v1.Instance.

    Examples
    =========

    .. doctest::
        >>> import mip
        >>> import ommx_python_mip_adapter as adapter

        >>> model = mip.Model()
        >>> x1=model.add_var(name="1", var_type=mip.INTEGER, lb=0, ub=5)
        >>> x2=model.add_var(name="2", var_type=mip.CONTINUOUS, lb=0, ub=5)

        >>> model.objective = - x1 - 2 * x2
        >>> constr = model.add_constr(x1 + x2 - 6 <= 0)

        >>> ommx_instance = adapter.model_to_instance(model)
    """
    builder = OMMXInstanceBuilder(model)
    return builder.build()
