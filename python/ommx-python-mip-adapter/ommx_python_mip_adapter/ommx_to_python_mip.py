from __future__ import annotations

from typing import Optional, final
from dataclasses import dataclass

import mip

from ommx.v1.function_pb2 import Function
from ommx.v1.solution_pb2 import Optimality
from ommx.v1 import Instance, DecisionVariable, Constraint, Solution

from .exception import OMMXPythonMIPAdapterError
from .python_mip_to_ommx import model_to_solution


@dataclass
class PythonMIPBuilder:
    """
    Build Python-MIP Model from ommx.v1.Instance.
    """

    instance: Instance
    model: mip.Model

    def __init__(
        self,
        instance: Instance,
        *,
        solver_name: str = mip.CBC,
        solver: Optional[mip.Solver] = None,
    ):
        if instance.raw.sense == Instance.MAXIMIZE:
            sense = mip.MAXIMIZE
        elif instance.raw.sense == Instance.MINIMIZE:
            sense = mip.MINIMIZE
        else:
            raise OMMXPythonMIPAdapterError(
                f"Not supported sense: {instance.raw.sense}"
            )
        self.instance = instance
        self.model = mip.Model(
            sense=sense,
            solver_name=solver_name,
            solver=solver,
        )

    def set_decision_variables(self):
        for var in self.instance.raw.decision_variables:
            if var.kind == DecisionVariable.BINARY:
                self.model.add_var(
                    name=str(var.id),
                    var_type=mip.BINARY,
                )
            elif var.kind == DecisionVariable.INTEGER:
                self.model.add_var(
                    name=str(var.id),
                    var_type=mip.INTEGER,
                    lb=var.bound.lower,  # type: ignore
                    ub=var.bound.upper,  # type: ignore
                )
            elif var.kind == DecisionVariable.CONTINUOUS:
                self.model.add_var(
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

    def as_lin_expr(
        self,
        f: Function,
    ) -> mip.LinExpr:
        """
        Translate ommx.v1.Function to `mip.LinExpr` or `float`.
        """
        if f.HasField("constant"):
            return mip.LinExpr(const=f.constant)  # type: ignore
        elif f.HasField("linear"):
            ommx_linear = f.linear
            return (
                mip.xsum(
                    term.coefficient * self.model.vars[str(term.id)]  # type: ignore
                    for term in ommx_linear.terms
                )
                + ommx_linear.constant
            )  # type: ignore
        raise OMMXPythonMIPAdapterError(
            "The function must be either `constant` or `linear`."
        )

    def set_objective(self):
        self.model.objective = self.as_lin_expr(self.instance.raw.objective)  # type: ignore

    def set_constraints(self):
        for constraint in self.instance.raw.constraints:
            lin_expr = self.as_lin_expr(constraint.function)
            if constraint.equality == Constraint.EQUAL_TO_ZERO:
                constr_expr = lin_expr == 0
            elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                constr_expr = lin_expr <= 0  # type: ignore
            else:
                raise OMMXPythonMIPAdapterError(
                    f"Not supported constraint equality: "
                    f"id: {constraint.id}, equality: {constraint.equality}"
                )
            self.model.add_constr(constr_expr, name=str(constraint.id))

    @final
    def build(self) -> mip.Model:
        self.set_decision_variables()
        self.set_objective()
        self.set_constraints()
        return self.model


def instance_to_model(
    instance: Instance,
    *,
    solver_name: str = mip.CBC,
    solver: Optional[mip.Solver] = None,
) -> mip.Model:
    """
    The function to convert ommx.v1.Instance to Python-MIP Model.

    Examples
    =========

    .. doctest::

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
        solver_name=solver_name,
        solver=solver,
    )
    return builder.build()


def solve(
    instance: Instance,
    *,
    relax: bool = False,
    solver_name: str = mip.CBC,
    solver: Optional[mip.Solver] = None,
) -> Solution:
    """
    Solve the given ommx.v1.Instance by Python-MIP, and return ommx.v1.Solution.

    :param instance: The ommx.v1.Instance to solve.
    :param relax: If True, relax all integer variables to continuous one by calling `Model.relax() <https://docs.python-mip.com/en/latest/classes.html#mip.Model.relax>`_ of Python-MIP.

    Examples
    =========

    .. doctest::

        >>> from ommx.v1 import Instance, DecisionVariable
        >>> from ommx_python_mip_adapter import solve

        KnapSack Problem

        >>> p = [10, 13, 18, 31, 7, 15]
        >>> w = [11, 15, 20, 35, 10, 33]
        >>> x = [DecisionVariable.binary(i) for i in range(6)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(p[i] * x[i] for i in range(6)),
        ...     constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
        ...     sense=Instance.MAXIMIZE,
        ... )

        Solve it

        >>> solution = solve(instance)

        Check output

        >>> sorted([(id, value) for id, value in solution.raw.state.entries.items()])
        [(0, 1.0), (1, 0.0), (2, 0.0), (3, 1.0), (4, 0.0), (5, 0.0)]
        >>> solution.raw.optimal
        True
        >>> solution.raw.feasible
        True

        p[0] + p[3] = 41
        w[0] + w[3] = 46 <= 47

        >>> solution.raw.objective
        41.0
        >>> solution.constraints["value"]
        id
        0   -1.0
        Name: value, dtype: float64

    """
    model = instance_to_model(instance, solver_name=solver_name, solver=solver)
    if relax:
        model.relax()
    model.optimize()

    state = model_to_solution(model, instance)
    solution = instance.evaluate(state)

    if model.status == mip.OptimizationStatus.OPTIMAL:
        solution.raw.optimality = Optimality.OPTIMALITY_OPTIMAL
    else:
        # TODO check the case where the solution is feasible but not optimal
        pass

    # TODO Store `relax` flag in the solution

    return solution
