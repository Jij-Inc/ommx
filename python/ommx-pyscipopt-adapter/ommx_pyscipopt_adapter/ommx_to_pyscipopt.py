import math

import pyscipopt

from ommx.v1 import Constraint, Instance, DecisionVariable, Solution
from ommx.v1.function_pb2 import Function
from ommx.v1.solution_pb2 import State

from .exception import OMMXPySCIPOptAdapterError


class OMMXSCIPAdapter:
    def __init__(self, instance: Instance):
        self._ommx_instance = instance.raw
        self._model = pyscipopt.Model()
        self._model.hideOutput()

    def _set_decision_variables(self):
        ommx_objective = self._ommx_instance.objective

        for var in self._ommx_instance.decision_variables:
            if var.kind == DecisionVariable.BINARY:
                self._model.addVar(
                    name=str(var.id),
                    vtype="B",
                )
            elif var.kind == DecisionVariable.INTEGER:
                self._model.addVar(
                    name=str(var.id),
                    vtype="I",
                    lb=var.bound.lower,
                    ub=var.bound.upper,
                )
            elif var.kind == DecisionVariable.CONTINUOUS:
                self._model.addVar(
                    name=str(var.id),
                    vtype="C",
                    lb=var.bound.lower,
                    ub=var.bound.upper,
                )
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Not supported decision variable kind: "
                    f"id: {var.id}, kind: {var.kind}"
                )
        if ommx_objective.HasField("quadratic"):
            # If objective function is quadratic, add the auxiliary variable for the linealized objective function,
            # because the setObjective method in PySCIPOpt does not support quadratic objective functions.
            self._model.addVar(
                name="auxiliary_for_linearized_objective", vtype="C", lb=None, ub=None
            )
        self._varname_to_var = {var.name: var for var in self._model.getVars()}

    def _make_linear_expr(
        self,
        ommx_function: Function,
    ) -> pyscipopt.Expr:
        ommx_linear = ommx_function.linear

        return (
            pyscipopt.quicksum(
                term.coefficient * self._varname_to_var[str(term.id)]
                for term in ommx_linear.terms
            )
            + ommx_linear.constant
        )

    def _make_quadratic_expr(
        self,
        ommx_function: Function,
    ) -> pyscipopt.Expr:
        ommx_quadratic = ommx_function.quadratic
        quadratic_term = pyscipopt.quicksum(
            self._varname_to_var[str(row)] * self._varname_to_var[str(column)] * value
            for row, column, value in zip(
                ommx_quadratic.rows, ommx_quadratic.columns, ommx_quadratic.values
            )
        )
        linear_term = pyscipopt.quicksum(
            term.coefficient * self._varname_to_var[str(term.id)]
            for term in ommx_quadratic.linear.terms
        )
        constant = ommx_quadratic.linear.constant
        return quadratic_term + linear_term + constant

    def _set_objective_function(self):
        ommx_objective = self._ommx_instance.objective
        if self._ommx_instance.sense == Instance.MAXIMIZE:
            sense = "maximize"
        elif self._ommx_instance.sense == Instance.MINIMIZE:
            sense = "minimize"
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Not supported sense: {self._ommx_instance.sense}"
            )

        if ommx_objective.HasField("constant"):
            self._model.setObjective(ommx_objective.constant, sense=sense)
        elif ommx_objective.HasField("linear"):
            expr = self._make_linear_expr(ommx_objective)
            self._model.setObjective(expr, sense=sense)
        elif ommx_objective.HasField("quadratic"):
            # The setObjective method in PySCIPOpt does not support quadratic objective functions.
            # So we introduce the auxiliary variable to linearize the objective function,
            # Example:
            #     input problem: min x^2 + y^2
            #
            #     introduce the auxiliary variable z, and the linearized objective function problem is:
            #         min z
            #         s.t. z >= x^2 + y^2
            auxilary_var = self._varname_to_var["auxiliary_for_linearized_objective"]

            # Add the auxiliary variable to the objective function.
            self._model.setObjective(auxilary_var, sense=sense)

            # Add the constraint for the auxiliary variable.
            expr = self._make_quadratic_expr(ommx_objective)
            if sense == "minimize":
                constr_expr = auxilary_var >= expr
            else:  # sense == "maximize"
                constr_expr = auxilary_var <= expr

            self._model.addCons(constr_expr, name="constraint_for_linearized_objective")

        else:
            raise OMMXPySCIPOptAdapterError(
                "The objective function must be `constant`, `linear`, `quadratic`."
            )

    def _set_constraints(self):
        ommx_constraints = self._ommx_instance.constraints

        for constraint in ommx_constraints:
            if constraint.function.HasField("linear"):
                expr = self._make_linear_expr(constraint.function)
            elif constraint.function.HasField("quadratic"):
                expr = self._make_quadratic_expr(constraint.function)
            elif constraint.function.HasField("constant"):
                if constraint.equality == Constraint.EQUAL_TO_ZERO and math.isclose(
                    constraint.function.constant, 0, abs_tol=1e-6
                ):
                    continue
                elif (
                    constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
                    and constraint.function.constant <= 1e-6
                ):
                    continue
                else:
                    raise OMMXPySCIPOptAdapterError(
                        f"Infeasible constant constraint was found:"
                        f"id: {constraint.id}"
                    )
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Constraints must be either `constant`, `linear` or `quadratic`."
                    f"id: {constraint.id}, "
                    f"type: {constraint.function.WhichOneof('function')}"
                )

            if constraint.equality == Constraint.EQUAL_TO_ZERO:
                constr_expr = expr == 0
            elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                constr_expr = expr <= 0
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Not supported constraint equality: "
                    f"id: {constraint.id}, equality: {constraint.equality}"
                )

            self._model.addCons(constr_expr, name=str(constraint.id))

    def build(self) -> pyscipopt.Model:
        self._set_decision_variables()
        self._set_objective_function()
        self._set_constraints()

        return self._model


def instance_to_model(instance: Instance) -> pyscipopt.Model:
    """
    Convert ommx.v1.Instance to pyscipopt.Model.

    Examples
    =========

    .. doctest::

        The following example shows how to create a pyscipopt.Model from an ommx.v1.Instance.

        >>> import ommx_pyscipopt_adapter as adapter
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

        >>> ommx_state = adapter.model_to_state(model, ommx_instance)
        >>> ommx_state.entries
        {1: -0.0}

    """

    builder = OMMXSCIPAdapter(instance)
    return builder.build()


def model_to_state(model: pyscipopt.Model, instance: Instance) -> State:
    """
    Convert optimized pyscipopt.Model and ommx.v1.Instance to ommx.v1.State.

    Examples
    =========

    .. doctest::

        The following example shows how to solve an unconstrained linear optimization problem with `x1` as the objective function.

        >>> import ommx_pyscipopt_adapter as adapter
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

        >>> ommx_state = adapter.model_to_state(model, ommx_instance)
        >>> ommx_state.entries
        {1: -0.0}

    """

    if model.getStatus() == "unknown":
        raise OMMXPySCIPOptAdapterError(
            "The model may not be optimized. [status: unknown]"
        )

    # NOTE: It is assumed that getBestSol will return an error
    #       if there is no feasible solution.
    try:
        sol = model.getBestSol()
        varname_to_var = {var.name: var for var in model.getVars()}
        return State(
            entries={
                var.id: sol[varname_to_var[str(var.id)]]
                for var in instance.raw.decision_variables
            }
        )
    except Exception:
        raise OMMXPySCIPOptAdapterError(
            f"There is no feasible solution. [status: {model.getStatus()}]"
        )


def model_to_solution(model: pyscipopt.Model, instance: Instance) -> Solution:
    """
    Convert optimized pyscipopt.Model and ommx.v1.Instance to ommx.v1.Solution.

    Examples
    =========

    .. doctest::

        >>> import ommx_pyscipopt_adapter as adapter
        >>> from ommx.v1 import Instance, DecisionVariable

        >>> p = [10, 13, 18, 31, 7, 15]
        >>> w = [11, 15, 20, 35, 10, 33]
        >>> x = [DecisionVariable.binary(i) for i in range(6)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(p[i] * x[i] for i in range(6)),
        ...     constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
        ...     sense=Instance.MAXIMIZE,
        ... )

        >>> model = adapter.instance_to_model(instance)
        >>> model.optimize()

        >>> solution = adapter.model_to_solution(model, instance)
        >>> solution.objective
        41.0

    """
    state = model_to_state(model, instance)
    solution = instance.evaluate(state)

    # TODO: Add the feature to store dual variables in `solution`.

    return solution
