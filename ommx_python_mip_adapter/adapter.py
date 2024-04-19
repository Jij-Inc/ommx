import typing as tp

import mip

from ommx.v1.constraint_pb2 import Constraint
from ommx.v1.decision_variables_pb2 import DecisionVariable
from ommx.v1.function_pb2 import Function
from ommx.v1.instance_pb2 import Instance
from ommx.v1.solution_pb2 import Solution, SolutionList

from ommx_python_mip_adapter.exception import OMMXPythonMIPAdapterError


class PythonMIPBuilder:
    def __init__(
        self,
        ommx_instance_bytes: bytes,
        *,
        sense: str = mip.MINIMIZE,
        solver_name: str = mip.CBC,
        solver: tp.Optional[mip.Solver] = None,
    ):
        try:
            self._ommx_instance = Instance.FromString(ommx_instance_bytes)
        except Exception as e:
            raise OMMXPythonMIPAdapterError(
                "Invalid `ommx_instance_bytes` as ommx.v1.Instance."
            ) from e

        self._model = mip.Model(
            sense=sense,
            solver_name=solver_name,
            solver=solver,
        )


    def _set_decision_variables(self):
        for var in self._ommx_instance.decision_variables:
            if var.kind == DecisionVariable.KIND_BINARY:
                self._model.add_var(
                    name=str(var.id),
                    var_type=mip.BINARY,
                )
            elif var.kind == DecisionVariable.KIND_INTEGER:
                self._model.add_var(
                    name=str(var.id),
                    var_type=mip.INTEGER,
                    lb=var.bound.lower,    # type: ignore
                    ub=var.bound.upper,    # type: ignore
                )
            elif var.kind == DecisionVariable.KIND_CONTINUOUS:
                self._model.add_var(
                    name=str(var.id),
                    var_type=mip.CONTINUOUS,
                    lb=var.bound.lower,    # type: ignore
                    ub=var.bound.upper,    # type: ignore
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

        return mip.xsum(
            term.coefficient * self._model.vars[str(term.id)] # type: ignore
            for term in ommx_linear.terms
        ) + ommx_linear.constant # type: ignore

            
    def _set_objective_function(self):
        ommx_objective = self._ommx_instance.objective

        if ommx_objective.HasField("constant"):
            self._model.objective = ommx_objective.constant    # type: ignore
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

            if constraint.equality == Constraint.EQUALITY_EQUAL_TO_ZERO:
                constr_expr = (lin_expr == 0)
            elif (constraint.equality == Constraint.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO):
                constr_expr = (lin_expr <= 0)    # type: ignore
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
    ommx_instance_bytes: bytes,
    *,
    sense: str = mip.MINIMIZE,
    solver_name: str = mip.CBC,
    solver: tp.Optional[mip.Solver] = None,
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
        >>> from ommx.v1.decision_variables_pb2 import DecisionVariable, Bound
        >>> from ommx.v1.instance_pb2 import Instance
        >>> from ommx.v1.function_pb2 import Function
        >>> from ommx.v1.linear_pb2 import Linear
        >>> from ommx.v1.solution_pb2 import SolutionList
        >>> ommx_instance = Instance(
        ...     decision_variables=[
        ...         DecisionVariable(
        ...             id=1,
        ...             kind=DecisionVariable.KIND_INTEGER,
        ...             bound=Bound(lower=0, upper=5),
        ...         ),
        ...     ],
        ...     objective=Function(
        ...         linear=Linear(
        ...             terms=[Linear.Term(id=1, coefficient=1)]
        ...         ),
        ...     ),
        ... )
        >>> ommx_instance_bytes = ommx_instance.SerializeToString()
        >>> model = adapter.instance_to_model(ommx_instance_bytes)
        >>> model.optimize()
        <OptimizationStatus.OPTIMAL: 0>
        >>> ommx_solutions_bytes = adapter.model_to_solution(
        ...     model, ommx_instance_bytes
        ... )
        >>> SolutionList.FromString(ommx_solutions_bytes).solutions[0].entries
        {1: 0.0}
    """
    builder = PythonMIPBuilder(
        ommx_instance_bytes,
        sense=sense,
        solver_name=solver_name,
        solver=solver,
    )
    return builder.build()


def model_to_solution(
    model: mip.Model,
    ommx_instance_bytes: bytes,
) -> bytes:
    """
    The function to create ommx.v1.SolutionList from optimized Python-MIP Model.

    Args:
        model (mip.Model): Optimized Python-MIP Model.
        ommx_instance_bytes (bytes): Serialized ommx.v1.Instance.

    Returns:
        bytes: Serialized ommx.v1.SolutionList

    Raises:
        OMMXPythonMIPAdapterError: When ommx.v1.SolutionList cannot be created.

    Examples:
        The following example of solving an unconstrained linear optimization problem with x1 as the objective function.

        >>> import ommx_python_mip_adapter as adapter
        >>> from ommx.v1.decision_variables_pb2 import DecisionVariable, Bound
        >>> from ommx.v1.instance_pb2 import Instance
        >>> from ommx.v1.function_pb2 import Function
        >>> from ommx.v1.linear_pb2 import Linear
        >>> from ommx.v1.solution_pb2 import SolutionList
        >>> ommx_instance = Instance(
        ...     decision_variables=[
        ...         DecisionVariable(
        ...             id=1,
        ...             kind=DecisionVariable.KIND_INTEGER,
        ...             bound=Bound(lower=0, upper=5),
        ...         ),
        ...     ],
        ...     objective=Function(
        ...         linear=Linear(
        ...             terms=[Linear.Term(id=1, coefficient=1)]
        ...         ),
        ...     ),
        ... )
        >>> ommx_instance_bytes = ommx_instance.SerializeToString()
        >>> model = adapter.instance_to_model(ommx_instance_bytes)
        >>> model.optimize()
        <OptimizationStatus.OPTIMAL: 0>
        >>> ommx_solutions_bytes = adapter.model_to_solution(
        ...     model, ommx_instance_bytes
        ... )
        >>> SolutionList.FromString(ommx_solutions_bytes).solutions[0].entries
        {1: 0.0}
    """
    if not (
        model.status == mip.OptimizationStatus.OPTIMAL
        or model.status == mip.OptimizationStatus.FEASIBLE
    ):
        raise OMMXPythonMIPAdapterError(
            "`model.status` must be `OPTIMAL` or `FEASIBLE`."
        )

    try:
        ommx_instance = Instance.FromString(ommx_instance_bytes)
    except Exception as e:
        raise OMMXPythonMIPAdapterError(
            "Invalid `ommx_instance_bytes` as ommx.v1.Instance."
        ) from e

    return SolutionList(
        solutions=[
            Solution(
                entries={
                    var.id: model.var_by_name(str(var.id)).x    # type: ignore
                    for var in ommx_instance.decision_variables
                }
            )
        ]
    ).SerializeToString()
