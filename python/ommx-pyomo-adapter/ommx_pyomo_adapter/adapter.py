from __future__ import annotations
from typing import Dict, Any, Optional
import math

import pyomo.environ as pyo
from pyomo.opt import TerminationCondition, SolverResults

from ommx.adapter import SolverAdapter, InfeasibleDetected, UnboundedDetected
from ommx.v1 import (
    Instance,
    Solution,
    DecisionVariable,
    Function,
    Constraint,
    State,
)

from .exception import OMMXPyomoAdapterError


class OMMXPyomoAdapter(SolverAdapter):
    solver_name: str
    solver_options: Dict[str, Any]

    def __init__(
        self,
        ommx_instance: Instance,
        *,
        solver_name: str = "cbc",
        solver_options: Optional[Dict[str, Any]] = None,
    ):
        """
        :param ommx_instance: The ommx.v1.Instance to solve.
        :param solver_name: The name of the solver to use (default: "cbc").
        :param solver_options: Optional solver options dictionary.
        """
        self.instance = ommx_instance
        self.solver_name = solver_name
        self.solver_options = solver_options or {}
        self.model = pyo.ConcreteModel()

        self._set_decision_variables()
        self._set_objective()
        self._set_constraints()

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        solver_name: str = "cbc",
        solver_options: Optional[Dict[str, Any]] = None,
    ) -> Solution:
        """
        Solve the given ommx.v1.Instance using Pyomo, returning an ommx.v1.Solution.

        :param ommx_instance: The ommx.v1.Instance to solve.
        :param solver_name: The name of the solver to use (default: "cbc").
        :param solver_options: Optional solver options dictionary.

        Examples
        =========

        KnapSack Problem

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> from ommx.v1 import Solution
            >>> from ommx_pyomo_adapter import OMMXPyomoAdapter

            >>> p = [10, 13, 18, 32, 7, 15]
            >>> w = [11, 15, 20, 35, 10, 33]
            >>> x = [DecisionVariable.binary(i) for i in range(6)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(p[i] * x[i] for i in range(6)),
            ...     constraints=[(sum(w[i] * x[i] for i in range(6)) <= 47).set_id(0)],
            ...     sense=Instance.MAXIMIZE,
            ... )

            Solve it

            >>> solution = OMMXPyomoAdapter.solve(instance)

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
            >>> solution.get_constraint_value(0)
            -1.0

        Infeasible Problem

        .. doctest::

                >>> from ommx.v1 import Instance, DecisionVariable
                >>> from ommx_pyomo_adapter import OMMXPyomoAdapter

                >>> x = DecisionVariable.integer(0, upper=3, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints=[x >= 4],
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> OMMXPyomoAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.adapter.InfeasibleDetected: Model was infeasible

        Unbounded Problem

        .. doctest::

                >>> from ommx.v1 import Instance, DecisionVariable
                >>> from ommx_pyomo_adapter import OMMXPyomoAdapter

                >>> x = DecisionVariable.integer(0, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints=[],
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> OMMXPyomoAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.adapter.UnboundedDetected: Model was unbounded
        """
        adapter = cls(ommx_instance, solver_name=solver_name, solver_options=solver_options)
        model = adapter.solver_input
        
        # Create solver
        try:
            solver = pyo.SolverFactory(solver_name)
            if not solver.available():
                raise OMMXPyomoAdapterError(f"Solver '{solver_name}' is not available")
        except Exception as e:
            raise OMMXPyomoAdapterError(f"Solver '{solver_name}' is not available")
        
        # Set solver options
        for key, value in adapter.solver_options.items():
            solver.options[key] = value
        
        # Solve
        results = solver.solve(model, tee=False)
        
        return adapter.decode(results)

    @property
    def solver_input(self) -> pyo.ConcreteModel:
        """The Pyomo ConcreteModel generated from this OMMX instance"""
        return self.model

    def decode(self, results: SolverResults) -> Solution:
        """Convert Pyomo SolverResults to ommx.v1.Solution.

        This method is intended to be used if the model has been acquired with
        `solver_input` for further adjustment of the solver parameters, and
        separately solving the model.

        Note that alterations to the model may make the decoding process
        incompatible -- decoding will only work if the model still describes
        effectively the same problem as the OMMX instance used to create the
        adapter.

        Examples
        =========

        .. doctest::

            >>> from ommx_pyomo_adapter import OMMXPyomoAdapter
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

            >>> adapter = OMMXPyomoAdapter(instance)
            >>> model = adapter.solver_input
            >>> # ... some modification of model's parameters
            >>> solver = pyo.SolverFactory("cbc")
            >>> results = solver.solve(model)

            >>> solution = adapter.decode(results)
            >>> solution.raw.objective
            42.0

        """
        # Check solver status and handle errors
        if results.solver.termination_condition == TerminationCondition.infeasible:
            raise InfeasibleDetected("Model was infeasible")
        
        if results.solver.termination_condition == TerminationCondition.unbounded:
            raise UnboundedDetected("Model was unbounded")
        
        if results.solver.termination_condition == TerminationCondition.infeasibleOrUnbounded:
            raise InfeasibleDetected("Model was infeasible or unbounded")

        state = self.decode_to_state(results)
        solution = self.instance.evaluate(state)

        # Set optimality based on termination condition
        if results.solver.termination_condition == TerminationCondition.optimal:
            solution.raw.optimality = Solution.OPTIMAL

        return solution

    def decode_to_state(self, results: SolverResults) -> State:
        """
        Create an ommx.v1.State from Pyomo SolverResults.

        Examples
        =========

        .. doctest::

            The following example shows how to solve an unconstrained linear optimization problem with `x1` as the objective function.

            >>> from ommx_pyomo_adapter import OMMXPyomoAdapter
            >>> from ommx.v1 import Instance, DecisionVariable

            >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
            >>> ommx_instance = Instance.from_components(
            ...     decision_variables=[x1],
            ...     objective=x1,
            ...     constraints=[],
            ...     sense=Instance.MINIMIZE,
            ... )
            >>> adapter = OMMXPyomoAdapter(ommx_instance)
            >>> model = adapter.solver_input
            >>> solver = pyo.SolverFactory("cbc")
            >>> results = solver.solve(model)

            >>> ommx_state = adapter.decode_to_state(results)
            >>> ommx_state.entries
            {1: 0.0}

        """
        # Check solver status and handle errors
        if results.solver.termination_condition == TerminationCondition.infeasible:
            raise InfeasibleDetected("Model was infeasible")
        
        if results.solver.termination_condition == TerminationCondition.unbounded:
            raise UnboundedDetected("Model was unbounded")
        
        if results.solver.termination_condition == TerminationCondition.infeasibleOrUnbounded:
            raise InfeasibleDetected("Model was infeasible or unbounded")

        try:
            entries = {}
            for var in self.instance.decision_variables:
                var_name = str(var.id)
                pyomo_var = getattr(self.model, var_name)
                value = pyo.value(pyomo_var)
                if value is None:
                    raise OMMXPyomoAdapterError(
                        f"Variable {var_name} has no value - model may not be solved"
                    )
                entries[var.id] = value
            return State(entries=entries)
        except Exception as e:
            raise OMMXPyomoAdapterError(
                f"Failed to decode state from results: {e}"
            )

    def _set_decision_variables(self):
        self.varname_map = {}
        
        for var in self.instance.decision_variables:
            var_name = str(var.id)
            
            if var.kind == DecisionVariable.BINARY:
                pyomo_var = pyo.Var(domain=pyo.Binary)
            elif var.kind == DecisionVariable.INTEGER:
                pyomo_var = pyo.Var(
                    domain=pyo.Integers,
                    bounds=(var.bound.lower, var.bound.upper)
                )
            elif var.kind == DecisionVariable.CONTINUOUS:
                pyomo_var = pyo.Var(
                    domain=pyo.Reals,
                    bounds=(var.bound.lower, var.bound.upper)
                )
            else:
                raise OMMXPyomoAdapterError(
                    f"Unsupported decision variable kind: "
                    f"id: {var.id}, kind: {var.kind}"
                )
            
            self.model.add_component(name=var_name, val=pyomo_var)
            self.varname_map[var_name] = pyomo_var

        # Check if objective is quadratic to add auxiliary variable
        degree = self.instance.objective.degree()
        if degree > 2:
            raise OMMXPyomoAdapterError(
                f"Objective function degree {degree} is not supported. "
                "Only constant, linear, and quadratic objectives are supported."
            )
        if degree == 2:
            # If objective function is quadratic, add the auxiliary variable for the linearized objective function,
            # because Pyomo quadratic objectives may need linearization for some solvers.
            aux_var = pyo.Var(domain=pyo.Reals)
            self.model.add_component(name="auxiliary_for_linearized_objective", val=aux_var)
            self.varname_map["auxiliary_for_linearized_objective"] = aux_var

    def _set_objective(self):
        if self.instance.sense == Instance.MAXIMIZE:
            sense = pyo.maximize
        elif self.instance.sense == Instance.MINIMIZE:
            sense = pyo.minimize
        else:
            raise OMMXPyomoAdapterError(
                f"Sense not supported: {self.instance.sense}"
            )

        objective = self.instance.objective
        degree = objective.degree()
        
        if degree == 0:
            obj_expr = objective.constant_term
        elif degree == 1:
            obj_expr = self._make_linear_expr(objective)
        elif degree == 2:
            # Pyomo does not always support quadratic objective functions for all solvers.
            # So we introduce the auxiliary variable to linearize the objective function,
            # Example:
            #     input problem: min x^2 + y^2
            #
            #     introduce the auxiliary variable z, and the linearized objective function problem is:
            #         min z
            #         s.t. z >= x^2 + y^2
            aux_var = self.varname_map["auxiliary_for_linearized_objective"]

            # Add the auxiliary variable to the objective function.
            obj_expr = aux_var

            # Add the constraint for the auxiliary variable.
            quad_expr = self._make_quadratic_expr(objective)
            if sense == pyo.minimize:
                constr_expr = aux_var >= quad_expr
            else:  # sense == pyo.maximize
                constr_expr = aux_var <= quad_expr

            self.model.add_component(
                name="constraint_for_linearized_objective", 
                val=pyo.Constraint(expr=constr_expr)
            )
        else:
            raise OMMXPyomoAdapterError(
                "The objective function must be constant, linear, or quadratic."
            )

        self.model.add_component(name="objective", val=pyo.Objective(expr=obj_expr, sense=sense))

    def _set_constraints(self):
        for constraint in self.instance.constraints:
            f = constraint.function
            degree = f.degree()
            
            if degree == 0:
                # Constant constraint is not passed to Pyomo, but checked for feasibility
                constant_value = f.constant_term
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
                    raise OMMXPyomoAdapterError(
                        f"Infeasible constant constraint was found: id {constraint.id}"
                    )
            elif degree == 1:
                expr = self._make_linear_expr(f)
            elif degree == 2:
                expr = self._make_quadratic_expr(f)
            else:
                raise OMMXPyomoAdapterError(
                    f"Constraints must be either constant, linear or quadratic. "
                    f"id: {constraint.id}, "
                    f"degree: {degree}"
                )

            if constraint.equality == Constraint.EQUAL_TO_ZERO:
                constr_expr = expr == 0
            elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                constr_expr = expr <= 0
            else:
                raise OMMXPyomoAdapterError(
                    f"Not supported constraint equality: "
                    f"id: {constraint.id}, equality: {constraint.equality}"
                )

            self.model.add_component(name=f"constraint_{constraint.id}", val=pyo.Constraint(expr=constr_expr))

    def _make_linear_expr(self, f: Function):
        expr = f.constant_term
        for var_id, coeff in f.linear_terms.items():
            expr += coeff * self.varname_map[str(var_id)]
        return expr

    def _make_quadratic_expr(self, f: Function):
        # Quadratic terms
        quad_expr = sum(
            coeff * self.varname_map[str(row)] * self.varname_map[str(col)]
            for (row, col), coeff in f.quadratic_terms.items()
        )

        # Linear terms
        linear_expr = sum(
            coeff * self.varname_map[str(var_id)]
            for var_id, coeff in f.linear_terms.items()
        )

        constant = f.constant_term

        return quad_expr + linear_expr + constant