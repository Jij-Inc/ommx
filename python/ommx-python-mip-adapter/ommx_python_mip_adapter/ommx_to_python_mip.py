from __future__ import annotations

from typing import Optional, final
from dataclasses import dataclass

import mip

from ommx.v1.function_pb2 import Function
from ommx.v1.solution_pb2 import Optimality, Result, Infeasible, Unbounded, Relaxation
from ommx.v1 import Instance, DecisionVariable, Constraint

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
        verbose: bool = False,
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
        if verbose:
            self.model.verbose = 1
        else:
            self.model.verbose = 0

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
    verbose: bool = False,
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
        verbose=verbose,
    )
    return builder.build()


def solve(
    instance: Instance,
    *,
    relax: bool = False,
    solver_name: str = mip.CBC,
    solver: Optional[mip.Solver] = None,
    verbose: bool = False,
) -> Result:
    """
    Solve the given ommx.v1.Instance by Python-MIP, and return ommx.v1.Solution.

    :param instance: The ommx.v1.Instance to solve.
    :param relax: If True, relax all integer variables to continuous one by calling `Model.relax() <https://docs.python-mip.com/en/latest/classes.html#mip.Model.relax>`_ of Python-MIP.

    Examples
    =========

    KnapSack Problem

    .. doctest::

        >>> from ommx.v1 import Instance, DecisionVariable
        >>> from ommx.v1.solution_pb2 import Optimality
        >>> from ommx_python_mip_adapter import solve

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

        >>> result = solve(instance)
        >>> solution = result.solution

        Check output

        >>> sorted([(id, value) for id, value in solution.state.entries.items()])
        [(0, 1.0), (1, 0.0), (2, 0.0), (3, 1.0), (4, 0.0), (5, 0.0)]
        >>> solution.feasible
        True
        >>> assert solution.optimality == Optimality.OPTIMALITY_OPTIMAL

        p[0] + p[3] = 41
        w[0] + w[3] = 46 <= 47

        >>> solution.objective
        41.0
        >>> solution.evaluated_constraints[0].evaluated_value
        -1.0

    Infeasible Problem

    .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> from ommx_python_mip_adapter import solve

            >>> x = DecisionVariable.integer(0, upper=3, lower=0)
            >>> instance = Instance.from_components(
            ...     decision_variables=[x],
            ...     objective=x,
            ...     constraints=[x >= 4],
            ...     sense=Instance.MAXIMIZE,
            ... )

            >>> result = solve(instance)
            >>> assert result.HasField("infeasible") is True
            >>> assert result.HasField("unbounded") is False
            >>> assert result.HasField("solution") is False

    Unbounded Problem

    .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> from ommx_python_mip_adapter import solve

            >>> x = DecisionVariable.integer(0, lower=0)
            >>> instance = Instance.from_components(
            ...     decision_variables=[x],
            ...     objective=x,
            ...     constraints=[],
            ...     sense=Instance.MAXIMIZE,
            ... )

            >>> result = solve(instance)
            >>> assert result.HasField("unbounded") is True
            >>> assert result.HasField("infeasible") is False
            >>> assert result.HasField("solution") is False

    Dual variable

    .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> from ommx_python_mip_adapter import solve

            >>> x = DecisionVariable.continuous(0, lower=0, upper=1)
            >>> y = DecisionVariable.continuous(1, lower=0, upper=1)
            >>> instance = Instance.from_components(
            ...     decision_variables=[x, y],
            ...     objective=x + y,
            ...     constraints=[x + y <= 1],
            ...     sense=Instance.MAXIMIZE,
            ... )

            >>> solution = solve(instance).solution
            >>> solution.evaluated_constraints[0].dual_variable
            1.0

    """
    model = instance_to_model(
        instance, solver_name=solver_name, solver=solver, verbose=verbose
    )
    if relax:
        model.relax()
    model.optimize()

    if model.status == mip.OptimizationStatus.INFEASIBLE:
        return Result(infeasible=Infeasible())

    if model.status == mip.OptimizationStatus.UNBOUNDED:
        return Result(unbounded=Unbounded())

    if model.status not in [
        mip.OptimizationStatus.OPTIMAL,
        mip.OptimizationStatus.FEASIBLE,
    ]:
        return Result(error=f"Unknown status: {model.status}")

    state = model_to_solution(model, instance)
    solution = instance.evaluate(state)

    assert solution.raw.feasible

    if model.status == mip.OptimizationStatus.OPTIMAL:
        solution.raw.optimality = Optimality.OPTIMALITY_OPTIMAL

    if relax:
        solution.raw.relaxation = Relaxation.RELAXATION_LP_RELAXED

    dual_variables = {}
    for constraint in model.constrs:
        pi = constraint.pi
        if pi is not None:
            id = int(constraint.name)
            dual_variables[id] = pi
    for constraint in solution.raw.evaluated_constraints:
        id = constraint.id
        if id in dual_variables:
            constraint.dual_variable = dual_variables[id]

    return Result(solution=solution.raw)
