from __future__ import annotations
from typing import Literal

import pyscipopt
import math

from ommx.adapter import SolverAdapter, InfeasibleDetected, UnboundedDetected
from ommx.v1 import Instance, Solution, DecisionVariable, Constraint
from ommx.v1.function_pb2 import Function
from ommx.v1.solution_pb2 import State, Optimality
from ommx.v1.constraint_hints_pb2 import ConstraintHints

from .exception import OMMXPySCIPOptAdapterError


HintMode = Literal["disabled", "auto", "forced"]


class OMMXPySCIPOptAdapter(SolverAdapter):
    use_sos1: HintMode

    def __init__(
        self,
        ommx_instance: Instance,
        *,
        use_sos1: Literal["disabled", "auto", "forced"] = "auto",
    ):
        self.instance = ommx_instance
        self.use_sos1 = use_sos1
        self.model = pyscipopt.Model()
        self.model.hideOutput()

        self._set_decision_variables()
        self._set_objective()
        self._set_constraints()

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        use_sos1: Literal["disabled", "auto", "forced"] = "auto",
    ) -> Solution:
        """
        Solve the given ommx.v1.Instance using PySCIPopt, returning an ommx.v1.Solution.

        :param ommx_instance: The ommx.v1.Instance to solve.

        Examples
        =========

        KnapSack Problem

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> from ommx.v1.solution_pb2 import Optimality
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
            >>> assert solution.optimality == Optimality.OPTIMALITY_OPTIMAL

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
        adapter = cls(ommx_instance, use_sos1=use_sos1)
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

        # there appears to be no enum for this in pyscipopt
        if data.getStatus() == "infeasible":
            raise InfeasibleDetected("Model was infeasible")

        if data.getStatus() == "unbounded":
            raise UnboundedDetected("Model was unbounded")

        # TODO: Add the feature to store dual variables in `solution`.

        state = self.decode_to_state(data)
        solution = self.instance.evaluate(state)

        if (
            data.getStatus() == "optimal"
        ):  # pyscipopt does not appear to have an enum or constant for this
            solution.raw.optimality = Optimality.OPTIMALITY_OPTIMAL

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

        # NOTE: It is assumed that getBestSol will return an error
        #       if there is no feasible solution.
        try:
            sol = data.getBestSol()
            # NOTE recreating the map instead of using `self.varname_map`, as
            # this is probably more robust.
            varname_map = {var.name: var for var in data.getVars()}
            return State(
                entries={
                    var.id: sol[varname_map[str(var.id)]]
                    for var in self.instance.raw.decision_variables
                }
            )
        except Exception:
            raise OMMXPySCIPOptAdapterError(
                f"There is no feasible solution. [status: {data.getStatus()}]"
            )

    def _set_decision_variables(self):
        for var in self.instance.raw.decision_variables:
            if var.kind == DecisionVariable.BINARY:
                self.model.addVar(name=str(var.id), vtype="B")
            elif var.kind == DecisionVariable.INTEGER:
                self.model.addVar(
                    name=str(var.id), vtype="I", lb=var.bound.lower, ub=var.bound.upper
                )
            elif var.kind == DecisionVariable.CONTINUOUS:
                self.model.addVar(
                    name=str(var.id), vtype="C", lb=var.bound.lower, ub=var.bound.upper
                )
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Unsupported decision variable kind: "
                    f"id: {var.id}, kind: {var.kind}"
                )

        if self.instance.raw.objective.HasField("quadratic"):
            # If objective function is quadratic, add the auxiliary variable for the linealized objective function,
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

        if objective.HasField("constant"):
            self.model.setObjective(objective.constant, sense=sense)
        elif objective.HasField("linear"):
            expr = self._make_linear_expr(objective)
            self.model.setObjective(expr, sense=sense)
        elif objective.HasField("quadratic"):
            # The setObjective method in PySCIPOpt does not support quadratic objective functions.
            # So we introduce the auxiliary variable to linearize the objective function,
            # Example:
            #     input problem: min x^2 + y^2
            #
            #     introduce the auxiliary variable z, and the linearized objective function problem is:
            #         min z
            #         s.t. z >= x^2 + y^2
            auxilary_var = self.varname_map["auxiliary_for_linearized_objective"]

            # Add the auxiliary variable to the objective function.
            self.model.setObjective(auxilary_var, sense=sense)

            # Add the constraint for the auxiliary variable.
            expr = self._make_quadratic_expr(objective)
            if sense == "minimize":
                constr_expr = auxilary_var >= expr
            else:  # sense == "maximize"
                constr_expr = auxilary_var <= expr

            self.model.addCons(constr_expr, name="constraint_for_linearized_objective")

        else:
            raise OMMXPySCIPOptAdapterError(
                "The objective function must be `constant`, `linear`, `quadratic`."
            )

        pass

    def _set_constraints(self):
        ommx_hints: ConstraintHints = self.instance.raw.constraint_hints

        excluded = set()

        if self.use_sos1 != "disabled":
            if self.use_sos1 == "force" and len(ommx_hints.sos1_constraints) == 0:
                raise OMMXPySCIPOptAdapterError(
                    "No SOS1 constraints were found, but `use_sos1` is set to `force`."
                )

            for sos1 in ommx_hints.sos1_constraints:
                bid = sos1.binary_constraint_id
                excluded.add(bid)
                big_m_ids = sos1.big_m_constraint_ids
                if len(big_m_ids) == 0:
                    name = f"sos1_{bid}"
                else:
                    name = f"sos1_{bid}_{'_'.join(map(str, big_m_ids))}"
                vars = [self.varname_map[str(v)] for v in sos1.decision_variables]
                self.model.addConsSOS1(vars, name=name)

        for constraint in self.instance.raw.constraints:
            if constraint.id in excluded:
                continue
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
                        f"Infeasible constant constraint was found: id {constraint.id}"
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

            self.model.addCons(constr_expr, name=str(constraint.id))

    def _make_linear_expr(self, function: Function) -> pyscipopt.Expr:
        linear = function.linear
        return (
            pyscipopt.quicksum(
                term.coefficient * self.varname_map[str(term.id)]
                for term in linear.terms
            )
            + linear.constant
        )

    def _make_quadratic_expr(self, function: Function) -> pyscipopt.Expr:
        quad = function.quadratic
        quad_terms = pyscipopt.quicksum(
            self.varname_map[str(row)] * self.varname_map[str(column)] * value
            for row, column, value in zip(quad.rows, quad.columns, quad.values)
        )

        linear_terms = pyscipopt.quicksum(
            term.coefficient * self.varname_map[str(term.id)]
            for term in quad.linear.terms
        )

        constant = quad.linear.constant

        return quad_terms + linear_terms + constant
