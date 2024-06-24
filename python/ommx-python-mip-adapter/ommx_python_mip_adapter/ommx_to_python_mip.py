from typing import Optional

import mip

from ommx.v1.constraint_pb2 import Equality
from ommx.v1.function_pb2 import Function
from ommx.v1 import Instance, DecisionVariable

from .exception import OMMXPythonMIPAdapterError


class PythonMIPBuilder:
    """
    Build Python-MIP Model from ommx.v1.Instance.
    """

    def __init__(
        self,
        instance: Instance,
        *,
        sense: str = mip.MINIMIZE,
        solver_name: str = mip.CBC,
        solver: Optional[mip.Solver] = None,
    ):
        self._ommx_instance = instance.raw
        self._model = mip.Model(
            sense=sense,
            solver_name=solver_name,
            solver=solver,
        )

    def _set_decision_variables(self):
        for var in self._ommx_instance.decision_variables:
            if var.kind == DecisionVariable.BINARY:
                self._model.add_var(
                    name=str(var.id),
                    var_type=mip.BINARY,
                )
            elif var.kind == DecisionVariable.INTEGER:
                self._model.add_var(
                    name=str(var.id),
                    var_type=mip.INTEGER,
                    lb=var.bound.lower,  # type: ignore
                    ub=var.bound.upper,  # type: ignore
                )
            elif var.kind == DecisionVariable.CONTINUOUS:
                self._model.add_var(
                    name=str(var.id),
                    var_type=mip.CONTINUOUS,
                    lb=var.bound.lower,  # type: ignore
                    ub=var.bound.upper,  # type: ignore
                )
            else:
                raise OMMXPythonMIPAdapterError(
                    f"Not supported decision variable kind: "
                    f"id: {var.id}, kind: {var.kind}"
                )

    def _make_linear_expr(
        self,
        ommx_function: Function,
    ) -> mip.LinExpr:
        ommx_linear = ommx_function.linear

        return (
            mip.xsum(
                term.coefficient * self._model.vars[str(term.id)]  # type: ignore
                for term in ommx_linear.terms
            )
            + ommx_linear.constant
        )  # type: ignore

    def _set_objective_function(self):
        ommx_objective = self._ommx_instance.objective

        if ommx_objective.HasField("constant"):
            self._model.objective = ommx_objective.constant  # type: ignore
        elif ommx_objective.HasField("linear"):
            self._model.objective = self._make_linear_expr(ommx_objective)
        else:
            raise OMMXPythonMIPAdapterError(
                "The objective function must be either `constant` or `linear`."
            )

    def _set_constraints(self):
        ommx_constraints = self._ommx_instance.constraints

        for constraint in ommx_constraints:
            if not constraint.function.HasField("linear"):
                raise OMMXPythonMIPAdapterError(
                    f"Only linear constraints are supported: "
                    f"id: {constraint.id}, "
                    f"type: {constraint.function.WhichOneof('function')}"
                )

            lin_expr = self._make_linear_expr(constraint.function)

            if constraint.equality == Equality.EQUALITY_EQUAL_TO_ZERO:
                constr_expr = lin_expr == 0
            elif constraint.equality == Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO:
                constr_expr = lin_expr <= 0  # type: ignore
            else:
                raise OMMXPythonMIPAdapterError(
                    f"Not supported constraint equality: "
                    f"id: {constraint.id}, equality: {constraint.equality}"
                )

            self._model.add_constr(constr_expr, name=str(constraint.id))

    def build(self) -> mip.Model:
        self._set_decision_variables()
        self._set_objective_function()
        self._set_constraints()

        return self._model


def instance_to_model(
    instance: Instance,
    *,
    sense: str = mip.MINIMIZE,
    solver_name: str = mip.CBC,
    solver: Optional[mip.Solver] = None,
) -> mip.Model:
    """
    The function to convert ommx.v1.Instance to Python-MIP Model.

    Args:
        ommx_instance_bytes (bytes): Serialized ommx.v1.Instance.
        sense (str): mip.MINIMIZE or mip.MAXIMIZE.
        solver_name (str): mip.CBC or mip.GUROBI. Searches for which solver is available if not informed.
        solver (mip.Solver): if this argument is provided, solver_name will be ignored.

    Returns:
        mip.Model: Python-MIP Model converted from ommx.v1.Instance.

    Raises:
        OMMXPythonMIPAdapterError: If converting is not possible.

    Examples:
        The following example of solving an unconstrained linear optimization problem with x1 as the objective function.

        >>> import ommx_python_mip_adapter as adapter
        >>> from ommx.v1 import Instance, DecisionVariable

        >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
        >>> ommx_instance = Instance.from_components(
        ...     decision_variables=[x1],
        ...     objective=x1,
        ...     constraints=[],
        ...     sense=Instance.MINIMIZE,
        ... )
        >>> model = adapter.instance_to_model(ommx_instance)
        >>> model.optimize()
        <OptimizationStatus.OPTIMAL: 0>

        >>> ommx_solutions = adapter.model_to_solution(model, ommx_instance)
        >>> ommx_solutions.entries
        {1: 0.0}
    """
    builder = PythonMIPBuilder(
        instance,
        sense=sense,
        solver_name=solver_name,
        solver=solver,
    )
    return builder.build()
