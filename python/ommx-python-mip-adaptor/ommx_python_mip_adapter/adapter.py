import typing as tp

import mip

from mip.exceptions import ParameterNotAvailable
from ommx.v1.constraint_pb2 import Constraint, Equality, ConstraintDescription
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1.solution_pb2 import State
from ommx.v1 import Instance, DecisionVariable

from ommx_python_mip_adapter.exception import OMMXPythonMIPAdapterError


class PythonMIPBuilder:
    def __init__(
        self,
        instance: Instance,
        *,
        sense: str = mip.MINIMIZE,
        solver_name: str = mip.CBC,
        solver: tp.Optional[mip.Solver] = None,
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


class OMMXInstanceBuilder:
    def __init__(
        self,
        model: mip.Model,
    ):
        self._model = model

    def _decision_variables(self) -> tp.List[DecisionVariable]:
        decision_variables = []

        for var in self._model.vars:
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
                    kind,
                    var.idx,
                    lower=var.lb,
                    upper=var.ub,
                    description=DecisionVariable.Description(name=var.name),
                )
            )

        return decision_variables

    def _make_function_from_lin_expr(
        self,
        lin_expr: mip.LinExpr,
    ) -> Function:
        terms = [
            Linear.Term(id=var.idx, coefficient=coeff)  # type: ignore
            for var, coeff in lin_expr.expr.items()
        ]
        constant: float = lin_expr.const  # type: ignore

        # If the terms are empty, the function is a constant.
        if len(terms) == 0:
            return Function(constant=constant)
        else:
            return Function(linear=Linear(terms=terms, constant=constant))

    def _objective(self) -> Function:
        # In Python-MIP, it is allowed not to set the objective function.
        # If it isn't set, the model behaves as if the objective function is set to 0.
        # However, an error occurs when accessing `.objective`.
        # So if an error occurs, treat the objective function as 0.
        try:
            objective = self._model.objective
        except ParameterNotAvailable:
            return Function(constant=0)

        return self._make_function_from_lin_expr(objective)

    def _constraints(self) -> tp.List[Constraint]:
        constraints = []

        for constr in self._model.constrs:
            id = constr.idx
            lin_expr = constr.expr
            name = constr.name

            if lin_expr.sense == "=":
                constraint = Constraint(
                    id=id,
                    equality=Equality.EQUALITY_EQUAL_TO_ZERO,
                    function=self._make_function_from_lin_expr(lin_expr),
                    description=ConstraintDescription(name=name),
                )
            elif lin_expr.sense == "<":
                constraint = Constraint(
                    id=id,
                    equality=Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO,
                    function=self._make_function_from_lin_expr(lin_expr),
                    description=ConstraintDescription(name=name),
                )
            elif lin_expr.sense == ">":
                # `ommx.v1.Constraint` does not support `GREATER_THAN_OR_EQUAL_TO_ZERO`.
                # So multiply the linear expression by -1.
                constraint = Constraint(
                    id=id,
                    equality=Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO,
                    function=self._make_function_from_lin_expr(-lin_expr),
                    description=ConstraintDescription(name=name),
                )
            else:
                raise OMMXPythonMIPAdapterError(
                    f"Not supported constraint sense: "
                    f"name: {constr.name}, sense: {lin_expr.sense}"
                )

            constraints.append(constraint)

        return constraints

    def _sense(self):
        if self._model.sense == mip.MAXIMIZE:
            return Instance.MAXIMIZE
        else:
            return Instance.MINIMIZE

    def build(self) -> Instance:
        return Instance.from_components(
            decision_variables=self._decision_variables(),
            objective=self._objective(),
            constraints=self._constraints(),
            sense=self._sense(),
        )


def instance_to_model(
    instance: Instance,
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
        >>> from ommx.v1.function_pb2 import Function
        >>> from ommx.v1.linear_pb2 import Linear
        >>> from ommx.v1 import Instance, DecisionVariable

        >>> ommx_instance = Instance.from_components(
        ...     decision_variables=[DecisionVariable.integer(1, lower=0, upper=5)],
        ...     objective=Function(
        ...         linear=Linear(
        ...             terms=[Linear.Term(id=1, coefficient=1)]
        ...         ),
        ...     ),
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


def model_to_instance(model: mip.Model) -> Instance:
    """
    The function to convert Python-MIP Model to ommx.v1.Instance.

    Args:
        model (mip.Model): Python-MIP Model.

    Returns:
        bytes: Serialized ommx.v1.Instance.

    Raises:
        OMMXPythonMIPAdapterError: If converting is not possible.

    Examples:
        >>> import mip
        >>> import ommx_python_mip_adapter as adapter
        >>> from ommx.v1.instance_pb2 import Instance
        >>> model = mip.Model()
        >>> x1=model.add_var(name="1", var_type=mip.INTEGER, lb=0, ub=5)
        >>> x2=model.add_var(name="2", var_type=mip.CONTINUOUS, lb=0, ub=5)
        >>> model.objective = - x1 - 2 * x2
        >>> constr = model.add_constr(x1 + x2 - 6 <= 0)
        >>> ommx_instance = adapter.model_to_instance(model)
    """
    builder = OMMXInstanceBuilder(model)
    return builder.build()


def model_to_solution(
    model: mip.Model,
    instance: Instance,
) -> State:
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
        >>> from ommx.v1.function_pb2 import Function
        >>> from ommx.v1.linear_pb2 import Linear
        >>> from ommx.v1 import Instance, DecisionVariable

        >>> ommx_instance = Instance.from_components(
        ...     decision_variables=[DecisionVariable.integer(1, lower=0, upper=5)],
        ...     objective=Function(
        ...         linear=Linear(
        ...             terms=[Linear.Term(id=1, coefficient=1)]
        ...         ),
        ...     ),
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
    if not (
        model.status == mip.OptimizationStatus.OPTIMAL
        or model.status == mip.OptimizationStatus.FEASIBLE
    ):
        raise OMMXPythonMIPAdapterError(
            "`model.status` must be `OPTIMAL` or `FEASIBLE`."
        )

    return State(
        entries={
            var.id: model.var_by_name(str(var.id)).x  # type: ignore
            for var in instance.raw.decision_variables
        }
    )
