ommx_pyscipopt_adapter.adapter
==============================

.. py:module:: ommx_pyscipopt_adapter.adapter


Classes
-------

.. autoapisummary::

   ommx_pyscipopt_adapter.adapter.OMMXPySCIPOptAdapter


Module Contents
---------------

.. py:class:: OMMXPySCIPOptAdapter(ommx_instance: ommx.v1.Instance, *, initial_state: Optional[ommx.v1.ToState] = None)



   An abstract interface for OMMX Solver Adapters, defining how solvers should be used with OMMX.

   See the `implementation guide <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_ for more details.

   Subclasses should set ``ADDITIONAL_CAPABILITIES`` to declare which non-standard
   constraint types they can handle. Standard constraints are always supported.

   Available capabilities:

   - ``AdditionalCapability.Indicator``: binvar = 1 → f(x) <= 0
   - ``AdditionalCapability.OneHot``: exactly one of a set of binary variables is 1
   - ``AdditionalCapability.Sos1``: at most one of a set of variables is non-zero

   The default is an empty set (standard constraints only).
   Subclasses must call ``super().__init__(ommx_instance)`` so that any
   constraint types the adapter does not support are automatically converted
   into regular constraints (Big-M for indicator / SOS1, linear equality for
   one-hot). Conversions mutate ``ommx_instance`` in place and are emitted
   at ``INFO`` level as ``tracing`` events from the Rust SDK; configure a
   Python OpenTelemetry ``TracerProvider`` before the first call to observe
   them via ``pyo3-tracing-opentelemetry``.


   .. py:method:: decode(data: pyscipopt.Model) -> ommx.v1.Solution

      Convert optimized pyscipopt.Model and ommx.v1.Instance to ommx.v1.Solution.

      This method is intended to be used if the model has been acquired with
      `solver_input` for further adjustment of the solver parameters, and
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
          ...     constraints={0: sum(w[i] * x[i] for i in range(6)) <= 47},
          ...     sense=Instance.MAXIMIZE,
          ... )

          >>> adapter = OMMXPySCIPOptAdapter(instance)
          >>> model = adapter.solver_input
          >>> # ... some modification of model's parameters
          >>> model.optimize()

          >>> solution = adapter.decode(model)
          >>> solution.objective
          42.0




   .. py:method:: decode_to_state(data: pyscipopt.Model) -> ommx.v1.State

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
          ...     constraints={},
          ...     sense=Instance.MINIMIZE,
          ... )
          >>> adapter = OMMXPySCIPOptAdapter(ommx_instance)
          >>> model = adapter.solver_input
          >>> model.optimize()

          >>> ommx_state = adapter.decode_to_state(model)
          >>> ommx_state.entries
          {1: 0.0}




   .. py:method:: solve(ommx_instance: ommx.v1.Instance, *, initial_state: Optional[ommx.v1.ToState] = None) -> ommx.v1.Solution
      :classmethod:


      Solve the given ommx.v1.Instance using PySCIPopt, returning an ommx.v1.Solution.

      :param ommx_instance: The ommx.v1.Instance to solve.
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
          ...     constraints={0: sum(w[i] * x[i] for i in range(6)) <= 47},
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
          >>> solution.get_constraint_value(0)
          -1.0

      Infeasible Problem

      .. doctest::

              >>> from ommx.v1 import Instance, DecisionVariable
              >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

              >>> x = DecisionVariable.integer(0, upper=3, lower=0)
              >>> instance = Instance.from_components(
              ...     decision_variables=[x],
              ...     objective=x,
              ...     constraints={0: x >= 4},
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
              ...     constraints={},
              ...     sense=Instance.MAXIMIZE,
              ... )

              >>> OMMXPySCIPOptAdapter.solve(instance)
              Traceback (most recent call last):
                  ...
              ommx.adapter.UnboundedDetected: Model was unbounded



   .. py:attribute:: ADDITIONAL_CAPABILITIES


   .. py:property:: solver_input
      :type: pyscipopt.Model


      The PySCIPOpt model generated from this OMMX instance



