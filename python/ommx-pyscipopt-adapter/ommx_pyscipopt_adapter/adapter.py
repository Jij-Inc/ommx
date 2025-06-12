from __future__ import annotations
from typing import Literal, Optional, Any

import pyscipopt
import math


from ommx.adapter import SolverAdapter, InfeasibleDetected, UnboundedDetected
from ommx.v1 import (
    Instance,
    Solution,
    DecisionVariable,
    Constraint,
    State,
    ToState,
    to_state,
)

from .exception import OMMXPySCIPOptAdapterError


HintMode = Literal["disabled", "auto", "forced"]


class OMMXPySCIPOptAdapter(SolverAdapter):
    use_sos1: HintMode

    def __init__(
        self,
        ommx_instance: Instance,
        *,
        use_sos1: Literal["disabled", "auto", "forced"] = "auto",
        initial_state: Optional[ToState] = None,
    ):
        """
        :param ommx_instance: The ommx.v1.Instance to solve.
        :param use_sos1: Strategy for handling SOS1 constraints.Options:
            - "disabled": Do not use SOS1 constraints.
            - "auto": Use SOS1 constraints if hints are provided, otherwise solve without them.(default)
            - "forced": Require SOS1 constraints and raise an error if no SOS1 constraint hints are found.
        :param initial_state: Optional initial solution state.
        """
        self.instance = ommx_instance
        self.use_sos1 = use_sos1
        self.model = pyscipopt.Model()
        self.model.hideOutput()

        self._set_decision_variables()
        self._set_objective()
        self._set_constraints()

        # Add initial solution if provided
        if initial_state is not None:
            self._add_initial_state(initial_state)

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        use_sos1: Literal["disabled", "auto", "forced"] = "auto",
        initial_state: Optional[ToState] = None,
    ) -> Solution:
        """
        Solve the given ommx.v1.Instance using PySCIPopt, returning an ommx.v1.Solution.

        :param ommx_instance: The ommx.v1.Instance to solve.
        :param use_sos1: Strategy for handling SOS1 constraints.Options:
            - "disabled": Do not use SOS1 constraints.
            - "auto": Use SOS1 constraints if hints are provided, otherwise solve without them.(default)
            - "forced": Require SOS1 constraints and raise an error if no SOS1 constraint hints are found.
        :param initial_state: Optional initial solution state.

        Examples
        =========

        KnapSack Problem

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> from ommx.v1 import Solution
            >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

            >>> p = [10, 13, 18, 32, 7, 15]
            >>> w = [11, 15, 20, 35, 10, 33]
            >>> x = [DecisionVariable.binary(i) for i in range(6)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(p[i] * x[i] for i in range(6)),
            ...     constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
            ...     sense=Instance.MAXIMIZE,
            ... )

            Solve it

            >>> solution = OMMXPySCIPOptAdapter.solve(instance)

            Check output

            >>> sorted([(id, value) for id, value in solution.state.entries.items()])
            [(0, 1.0), (1, 0.0), (2, 0.0), (3, 1.0), (4, 0.0), (5, 0.0)]
            >>> solution.feasible
            True
            >>> assert solution.optimality == Solution.OPTIMAL

            p[0] + p[3] = 42
            w[0] + w[3] = 46 <= 47

            >>> solution.objective
            42.0
            >>> solution.raw.evaluated_constraints[0].evaluated_value
            -1.0

        Infeasible Problem

        .. doctest::

                >>> from ommx.v1 import Instance, DecisionVariable
                >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

                >>> x = DecisionVariable.integer(0, upper=3, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints=[x >= 4],
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> OMMXPySCIPOptAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.adapter.InfeasibleDetected: Model was infeasible

        Unbounded Problem

        .. doctest::

                >>> from ommx.v1 import Instance, DecisionVariable
                >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

                >>> x = DecisionVariable.integer(0, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints=[],
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> OMMXPySCIPOptAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.adapter.UnboundedDetected: Model was unbounded
        """
        adapter = cls(ommx_instance, use_sos1=use_sos1, initial_state=initial_state)
        model = adapter.solver_input
        model.optimize()
        return adapter.decode(model)

    @property
    def solver_input(self) -> pyscipopt.Model:
        """The PySCIPOpt model generated from this OMMX instance"""
        return self.model

    def decode(self, data: pyscipopt.Model) -> Solution:
        """Convert optimized pyscipopt.Model and ommx.v1.Instance to ommx.v1.Solution.

        This method is intended to be used if the model has been acquired with
        `solver_input` for futher adjustment of the solver parameters, and
        separately optimizing the model.

        Note that alterations to the model may make the decoding process
        incompatible -- decoding will only work if the model still describes
        effectively the same problem as the OMMX instance used to create the
        adapter.

        Examples
        =========

        .. doctest::

            >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
            >>> from ommx.v1 import Instance, DecisionVariable

            >>> p = [10, 13, 18, 32, 7, 15]
            >>> w = [11, 15, 20, 35, 10, 33]
            >>> x = [DecisionVariable.binary(i) for i in range(6)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(p[i] * x[i] for i in range(6)),
            ...     constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
            ...     sense=Instance.MAXIMIZE,
            ... )

            >>> adapter = OMMXPySCIPOptAdapter(instance)
            >>> model = adapter.solver_input
            >>> # ... some modification of model's parameters
            >>> model.optimize()

            >>> solution = adapter.decode(model)
            >>> solution.raw.objective
            42.0

        """

        # TODO: Add the feature to store dual variables in `solution`.

        state = self.decode_to_state(data)
        solution = self.instance.evaluate(state)

        if (
            data.getStatus() == "optimal"
        ):  # pyscipopt does not appear to have an enum or constant for this
            solution.raw.optimality = Solution.OPTIMAL

        return solution

    def decode_to_state(self, data: pyscipopt.Model) -> State:
        """
        Create an ommx.v1.State from an optimized PySCIPOpt Model.

        Examples
        =========

        .. doctest::

            The following example shows how to solve an unconstrained linear optimization problem with `x1` as the objective function.

            >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
            >>> from ommx.v1 import Instance, DecisionVariable

            >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
            >>> ommx_instance = Instance.from_components(
            ...     decision_variables=[x1],
            ...     objective=x1,
            ...     constraints=[],
            ...     sense=Instance.MINIMIZE,
            ... )
            >>> adapter = OMMXPySCIPOptAdapter(ommx_instance)
            >>> model = adapter.solver_input
            >>> model.optimize()

            >>> ommx_state = adapter.decode_to_state(model)
            >>> ommx_state.entries
            {1: -0.0}

        """
        if data.getStatus() == "unknown":
            raise OMMXPySCIPOptAdapterError(
                "The model may not be optimized. [status: unknown]"
            )

        if data.getStatus() == "infeasible":
            raise InfeasibleDetected("Model was infeasible")

        if data.getStatus() == "unbounded":
            raise UnboundedDetected("Model was unbounded")

        if data.getStatus() == "timelimit":
            # The following condition checks if there is no feasible primal solution.
            # In other words, it is checking for the absence of any feasible solution.
            if data.getNSols() == 0:
                raise InfeasibleDetected("Model was infeasible [status: timelimit]")

        # NOTE: It is assumed that getBestSol will return an error
        #       if there is no feasible solution.
        try:
            sol = data.getBestSol()
            # NOTE recreating the map instead of using `self.varname_map`, as
            # this is probably more robust.
            varname_map = {var.name: var for var in data.getVars()}
            return State(
                entries={
                    var_id: sol[varname_map[str(var_id)]]
                    for var_id, _ in self.instance.raw.decision_variables.items()
                }
            )
        except Exception:
            raise OMMXPySCIPOptAdapterError(
                f"There is no feasible solution. [status: {data.getStatus()}]"
            )

    def _set_decision_variables(self):
        # Use .raw to access the dict directly
        for var_id, var in self.instance.raw.decision_variables.items():
            if var.kind == DecisionVariable.BINARY:
                self.model.addVar(name=str(var_id), vtype="B")
            elif var.kind == DecisionVariable.INTEGER:
                self.model.addVar(
                    name=str(var_id), vtype="I", lb=var.bound.lower, ub=var.bound.upper
                )
            elif var.kind == DecisionVariable.CONTINUOUS:
                self.model.addVar(
                    name=str(var_id), vtype="C", lb=var.bound.lower, ub=var.bound.upper
                )
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Unsupported decision variable kind: "
                    f"id: {var_id}, kind: {var.kind}"
                )

        # Check if objective is quadratic to add auxiliary variable
        objective_func = self.instance.raw.objective
        if objective_func.as_quadratic() is not None:
            # If objective function is quadratic, add the auxiliary variable for the linearized objective function,
            # because the setObjective method in PySCIPOpt does not support quadratic objective functions.
            self.model.addVar(
                name="auxiliary_for_linearized_objective", vtype="C", lb=None, ub=None
            )

        self.varname_map = {var.name: var for var in self.model.getVars()}

    def _set_objective(self):
        objective = self.instance.raw.objective

        if self.instance.sense == Instance.MAXIMIZE:
            sense = "maximize"
        elif self.instance.sense == Instance.MINIMIZE:
            sense = "minimize"
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Sense not supported: {self.instance.sense}"
            )

        # Handle constant objective
        if objective.degree() == 0:
            # It's a constant function
            linear_func = objective.as_linear()
            if linear_func is not None:
                self.model.setObjective(linear_func.constant_term, sense=sense)
            else:
                self.model.setObjective(0.0, sense=sense)
        # Handle linear objective
        elif objective.as_linear() is not None:
            expr = self._make_linear_expr(objective)
            self.model.setObjective(expr, sense=sense)
        # Handle quadratic objective
        elif objective.as_quadratic() is not None:
            # The setObjective method in PySCIPOpt does not support quadratic objective functions.
            # So we introduce the auxiliary variable to linearize the objective function,
            # Example:
            #     input problem: min x^2 + y^2
            #
            #     introduce the auxiliary variable z, and the linearized objective function problem is:
            #         min z
            #         s.t. z >= x^2 + y^2
            auxiliary_var = self.varname_map["auxiliary_for_linearized_objective"]

            # Add the auxiliary variable to the objective function.
            self.model.setObjective(auxiliary_var, sense=sense)

            # Add the constraint for the auxiliary variable.
            expr = self._make_quadratic_expr(objective)
            if sense == "minimize":
                constr_expr = auxiliary_var >= expr
            else:  # sense == "maximize"
                constr_expr = auxiliary_var <= expr

            self.model.addCons(constr_expr, name="constraint_for_linearized_objective")
        else:
            raise OMMXPySCIPOptAdapterError(
                "The objective function must be constant, linear, or quadratic."
            )

    def _set_constraints(self):
        # TODO: Re-implement constraint hints after migration (SOS1 constraints)
        # For now, skip SOS1 constraint handling
        excluded = set()

        # TODO: Re-implement SOS1 constraint handling after migration
        # Skip SOS1 constraints for now

        for constraint_id, constraint in self.instance.raw.constraints.items():
            if constraint_id in excluded:
                continue

            # Handle constraint function based on its type
            constraint_func = constraint.function

            # Try to create expression based on function type
            expr = None
            if constraint_func.as_linear() is not None:
                expr = self._make_linear_expr(constraint_func)
            elif constraint_func.as_quadratic() is not None:
                expr = self._make_quadratic_expr(constraint_func)
            elif constraint_func.degree() == 0:
                # Constant constraint - handle specially
                linear_func = constraint_func.as_linear()
                if linear_func is not None:
                    constant_value = linear_func.constant_term
                    if constraint.equality == Constraint.EQUAL_TO_ZERO and math.isclose(
                        constant_value, 0, abs_tol=1e-6
                    ):
                        continue  # Skip feasible constant constraint
                    elif (
                        constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
                        and constant_value <= 1e-6
                    ):
                        continue  # Skip feasible constant constraint
                    else:
                        raise OMMXPySCIPOptAdapterError(
                            f"Infeasible constant constraint was found: id {constraint_id}"
                        )
                else:
                    continue  # Skip if can't get constant value
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Constraints must be either constant, linear or quadratic. "
                    f"id: {constraint_id}, "
                    f"degree: {constraint_func.degree()}"
                )

            # Only add constraint if we have a valid expression
            if expr is not None:
                if constraint.equality == Constraint.EQUAL_TO_ZERO:
                    constr_expr = expr == 0
                elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                    constr_expr = expr <= 0
                else:
                    raise OMMXPySCIPOptAdapterError(
                        f"Not supported constraint equality: "
                        f"id: {constraint_id}, equality: {constraint.equality}"
                    )

                self.model.addCons(constr_expr, name=str(constraint_id))

            if constraint.equality == Constraint.EQUAL_TO_ZERO:
                constr_expr = expr == 0
            elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                constr_expr = expr <= 0
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Not supported constraint equality: "
                    f"id: {constraint.id}, equality: {constraint.equality}"
                )

            self.model.addCons(constr_expr, name=str(constraint.id))

    def _make_linear_expr(self, function: Any) -> pyscipopt.Expr:
        linear = function.as_linear()
        if linear is None:
            raise OMMXPySCIPOptAdapterError("Function is not linear")

        return (
            pyscipopt.quicksum(
                coeff * self.varname_map[str(var_id)]
                for var_id, coeff in linear.linear_terms.items()
            )
            + linear.constant_term
        )

    def _make_quadratic_expr(self, function: Any) -> pyscipopt.Expr:
        quad = function.as_quadratic()
        if quad is None:
            raise OMMXPySCIPOptAdapterError("Function is not quadratic")

        # Quadratic terms
        quad_terms = pyscipopt.quicksum(
            self.varname_map[str(row)] * self.varname_map[str(col)] * coeff
            for (row, col), coeff in quad.quadratic_terms.items()
        )

        # Linear terms
        linear_terms = pyscipopt.quicksum(
            coeff * self.varname_map[str(var_id)]
            for var_id, coeff in quad.linear_terms.items()
        )

        constant = quad.constant_term

        return quad_terms + linear_terms + constant

    def _add_initial_state(self, initial_state: ToState) -> None:
        initial_sol = self.model.createSol()
        for var_id, value in to_state(initial_state).entries.items():
            var_name = str(var_id)
            if var_name in self.varname_map:
                self.model.setSolVal(initial_sol, self.varname_map[var_name], value)
        # The free=True parameter means that solution will be freed afterwards.
        self.model.addSol(initial_sol, free=True)
