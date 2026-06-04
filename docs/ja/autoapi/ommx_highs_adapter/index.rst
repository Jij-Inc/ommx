ommx_highs_adapter
==================

.. py:module:: ommx_highs_adapter


Submodules
----------

.. toctree::
   :maxdepth: 1

   /autoapi/ommx_highs_adapter/adapter/index
   /autoapi/ommx_highs_adapter/exception/index


Exceptions
----------

.. autoapisummary::

   ommx_highs_adapter.OMMXHighsAdapterError


Classes
-------

.. autoapisummary::

   ommx_highs_adapter.OMMXHighsAdapter


Package Contents
----------------

.. py:exception:: OMMXHighsAdapterError



   Common base class for all non-exit exceptions.


.. py:class:: OMMXHighsAdapter(ommx_instance: ommx.v1.Instance, *, verbose: bool = False)



   OMMX Adapter for HiGHS solver.

   This adapter translates OMMX optimization problems (ommx.v1.Instance) into HiGHS-compatible
   formats and converts HiGHS solutions back to OMMX format (ommx.v1.Solution).

   Translation Specifications
   ==========================

   Decision Variables
   ------------------
   The adapter handles the following translations for decision variables:

   **ID Management**:

   - OMMX: Variables managed by IDs (non-sequential integers)
   - HiGHS: Variables managed by array indices (0-based sequential)
   - Mapping maintained internally for bidirectional conversion

   **Variable Types**:

   .. list-table::
      :header-rows: 1

      * - OMMX Type
        - HiGHS Type
        - Bounds
      * - ``DecisionVariable.BINARY``
        - ``HighsVarType.kInteger``
        - ``[0, 1]``
      * - ``DecisionVariable.INTEGER``
        - ``HighsVarType.kInteger``
        - ``[var.bound.lower, var.bound.upper]``
      * - ``DecisionVariable.CONTINUOUS``
        - ``HighsVarType.kContinuous``
        - ``[var.bound.lower, var.bound.upper]``
      * - ``DecisionVariable.SEMI_INTEGER``
        - **Not supported** (support planned)
        - \-
      * - ``DecisionVariable.SEMI_CONTINUOUS``
        - **Not supported** (support planned)
        - \-

   **Note**: Semi-integer and semi-continuous variables are planned for future support but are
   currently unsupported. Using these variable types will raise an ``OMMXHighsAdapterError``.

   Constraints
   -----------
   **Supported Function Types**:

   - Constant functions (ommx.v1.Function.constant)
   - Linear functions (ommx.v1.Function.linear)

   **Constraint Types**:

   .. list-table::
      :header-rows: 1

      * - OMMX Constraint
        - Mathematical Form
        - HiGHS Constraint
      * - ``Constraint.EQUAL_TO_ZERO``
        - f(x) = 0
        - ``const_expr == 0``
      * - ``Constraint.LESS_THAN_OR_EQUAL_TO_ZERO``
        - f(x) ≤ 0
        - ``const_expr <= 0``

   **Constant Constraint Handling**:

   - Equality: Skip if \|constant\| ≤ 1e-10, error if \|constant\| > 1e-10
   - Inequality: Skip if constant ≤ 1e-10, error if constant > 1e-10

   **Constraint ID Management**:

   - OMMX constraint IDs converted to HiGHS constraint names via ``str(constraint.id)``

   Objective Function
   ------------------
   **Optimization Direction**:

   .. list-table::
      :header-rows: 1

      * - OMMX Direction
        - HiGHS Method
      * - ``Instance.MINIMIZE``
        - ``model.minimize(...)``
      * - ``Instance.MAXIMIZE``
        - ``model.maximize(...)``

   **Function Types**:

   - Constant objectives: Processing skipped
   - Linear objectives: Converted to HiGHS linear expressions

   Solution Decoding
   -----------------
   **Variable Values**: Extracted from HiGHS ``solution.col_value`` using maintained ID mapping

   **Optimality Status**: Set to ``OPTIMALITY_OPTIMAL`` when HiGHS returns ``kOptimal``

   **Dual Variables**: Extracted from ``solution.row_dual`` for constraints

   Error Handling
   --------------
   **Unsupported Features**:

   - Quadratic functions (HiGHS supports linear problems only)
   - Semi-integer variables (``DecisionVariable.SEMI_INTEGER``, kind=4) - support planned
   - Semi-continuous variables (``DecisionVariable.SEMI_CONTINUOUS``, kind=5) - support planned
   - Constraint types other than ``EQUAL_TO_ZERO``/``LESS_THAN_OR_EQUAL_TO_ZERO``

   **Solver Status Mapping**:

   .. list-table::
      :header-rows: 1

      * - HiGHS Status
        - Exception
      * - ``kInfeasible``
        - ``InfeasibleDetected``
      * - ``kUnbounded``
        - ``UnboundedDetected``
      * - ``kNotset``
        - ``OMMXHighsAdapterError``

   Limitations
   -----------
   1. Linear problems only (no quadratic constraints or objectives)
   2. Constraint forms limited to equality (= 0) and inequality (≤ 0)
   3. Variable types limited to Binary, Integer, and Continuous

      - Semi-integer (SEMI_INTEGER) support is planned but not yet implemented
      - Semi-continuous (SEMI_CONTINUOUS) support is planned but not yet implemented

   Examples
   --------
   >>> from ommx_highs_adapter import OMMXHighsAdapter
   >>> from ommx.v1 import Instance, DecisionVariable
   >>>
   >>> # Define problem
   >>> x = DecisionVariable.binary(0)
   >>> y = DecisionVariable.integer(1, lower=0, upper=10)
   >>> instance = Instance.from_components(
   ...     decision_variables=[x, y],
   ...     objective=2*x + 3*y,
   ...     constraints={0: x + y <= 5},
   ...     sense=Instance.MAXIMIZE,
   ... )
   >>>
   >>> # Solve
   >>> solution = OMMXHighsAdapter.solve(instance)
   >>> print(f"Optimal value: {solution.objective}")
   Optimal value: 15.0
   >>> print(f"Variables: {solution.state.entries}")
   Variables: {0: 0.0, 1: 5.0}



   .. py:method:: decode(data: highspy.Highs) -> ommx.v1.Solution

      Convert an optimized HiGHS model back to an OMMX Solution.

      This method translates HiGHS solver results into OMMX format, including
      variable values, optimality status, and dual variable information.

      Parameters
      ----------
      data : highspy.Highs
          The HiGHS model that has been optimized. Must be the same model
          returned by solver_input property.

      Returns
      -------
      Solution
          Complete OMMX solution containing:
          - Variable values mapped back to original OMMX IDs
          - Constraint evaluations and feasibility status
          - Optimality information from HiGHS
          - Dual variables for linear constraints

      Raises
      ------
      OMMXHighsAdapterError
          If the model has not been optimized yet
      InfeasibleDetected
          If HiGHS determined the problem is infeasible
      UnboundedDetected
          If HiGHS determined the problem is unbounded

      Notes
      -----
      This method should only be used after solving the model with HiGHS.
      Any modifications to the HiGHS model structure after creation may
      make the decoding process incompatible.

      The dual variables are extracted from HiGHS's row_dual and mapped
      to OMMX constraints based on their order. Only constraints with
      valid dual information will have the dual_variable field set.

      Examples
      --------
      >>> from ommx_highs_adapter import OMMXHighsAdapter
      >>> from ommx.v1 import Instance, DecisionVariable
      >>>
      >>> x = DecisionVariable.binary(0)
      >>> instance = Instance.from_components(
      ...     decision_variables=[x],
      ...     objective=x,
      ...     constraints={},
      ...     sense=Instance.MAXIMIZE,
      ... )
      >>>
      >>> adapter = OMMXHighsAdapter(instance)
      >>> model = adapter.solver_input
      >>> model.run()  # doctest: +ELLIPSIS
      <...>
      >>> solution = adapter.decode(model)
      >>> solution.objective
      1.0



   .. py:method:: decode_to_state(data: highspy.Highs) -> ommx.v1.State

      Extract variable values from an optimized HiGHS model as an OMMX State.

      Parameters
      ----------
      data : highspy.Highs
          The optimized HiGHS model

      Returns
      -------
      State
          OMMX state containing variable values mapped to original OMMX IDs

      Raises
      ------
      OMMXHighsAdapterError
          If the model has not been optimized
      InfeasibleDetected
          If the model is infeasible
      UnboundedDetected
          If the model is unbounded

      Examples
      --------
      >>> from ommx_highs_adapter import OMMXHighsAdapter
      >>> from ommx.v1 import Instance, DecisionVariable
      >>>
      >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
      >>> instance = Instance.from_components(
      ...     decision_variables=[x1],
      ...     objective=x1,
      ...     constraints={},
      ...     sense=Instance.MINIMIZE,
      ... )
      >>> adapter = OMMXHighsAdapter(instance)
      >>> model = adapter.solver_input
      >>> model.run()  # doctest: +ELLIPSIS
      <...>
      >>> state = adapter.decode_to_state(model)
      >>> state.entries
      {1: 0.0}



   .. py:method:: solve(ommx_instance: ommx.v1.Instance, *, verbose: bool = False) -> ommx.v1.Solution
      :classmethod:


      Solve an OMMX optimization problem using HiGHS solver.

      This method provides a convenient interface for solving optimization problems
      without needing to manually instantiate the adapter. It handles the complete
      workflow: translation to HiGHS format, solving, and result conversion.

      Parameters
      ----------
      ommx_instance : Instance
          The OMMX optimization problem to solve. Must satisfy HiGHS adapter
          requirements: linear objective function (constant or linear terms only),
          linear constraints (constant or linear terms only), variables of type
          Binary, Integer, or Continuous only (Semi-integer and Semi-continuous
          support is planned but not yet implemented), and constraints of type
          ``EQUAL_TO_ZERO`` or ``LESS_THAN_OR_EQUAL_TO_ZERO`` only.

      verbose : bool, default=False
          If True, enable HiGHS's console logging for debugging

      Returns
      -------
      Solution
          The solution containing:
          - Variable values in solution.state.entries
          - Objective value in solution.objective
          - Constraint evaluations in solution.constraints
          - Optimality status in solution.optimality
          - Dual variables (if available) in constraint.dual_variable

      Raises
      ------
      InfeasibleDetected
          When the optimization problem has no feasible solution
      UnboundedDetected
          When the optimization problem is unbounded
      OMMXHighsAdapterError
          When the problem contains unsupported features or HiGHS encounters an error

      Examples
      --------
      **Knapsack Problem**

      >>> from ommx.v1 import Instance, DecisionVariable, Solution
      >>> from ommx_highs_adapter import OMMXHighsAdapter
      >>>
      >>> p = [10, 13, 18, 32, 7, 15]  # profits
      >>> w = [11, 15, 20, 35, 10, 33]  # weights
      >>> x = [DecisionVariable.binary(i) for i in range(6)]
      >>> instance = Instance.from_components(
      ...     decision_variables=x,
      ...     objective=sum(p[i] * x[i] for i in range(6)),
      ...     constraints={0: sum(w[i] * x[i] for i in range(6)) <= 47},
      ...     sense=Instance.MAXIMIZE,
      ... )
      >>>
      >>> solution = OMMXHighsAdapter.solve(instance)
      >>> sorted([(id, value) for id, value in solution.state.entries.items()])
      [(0, 1.0), (1, 0.0), (2, 0.0), (3, 1.0), (4, 0.0), (5, 0.0)]
      >>> solution.feasible
      True
      >>> assert solution.optimality == Solution.OPTIMAL
      >>> solution.objective
      42.0

      **Infeasible Problem**

      >>> x = DecisionVariable.integer(0, upper=3, lower=0)
      >>> instance = Instance.from_components(
      ...     decision_variables=[x],
      ...     objective=x,
      ...     constraints={0: x >= 4},  # Impossible: x ≤ 3 and x ≥ 4
      ...     sense=Instance.MAXIMIZE,
      ... )
      >>> OMMXHighsAdapter.solve(instance)  # doctest: +IGNORE_EXCEPTION_DETAIL
      Traceback (most recent call last):
          ...
      ommx.adapter.InfeasibleDetected: Model was infeasible



   .. py:attribute:: ADDITIONAL_CAPABILITIES
      :type:  frozenset[ommx.v1.AdditionalCapability]


   .. py:property:: solver_input
      :type: highspy.Highs


      The HiGHS model generated from the OMMX instance.

      Returns
      -------
      highspy.Highs
          The HiGHS model ready for optimization. This model contains:
          - Decision variables translated from OMMX IDs to HiGHS indices
          - Constraints converted to HiGHS linear expressions
          - Objective function set according to optimization direction



