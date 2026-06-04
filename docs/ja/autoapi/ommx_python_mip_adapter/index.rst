ommx_python_mip_adapter
=======================

.. py:module:: ommx_python_mip_adapter


Submodules
----------

.. toctree::
   :maxdepth: 1

   /autoapi/ommx_python_mip_adapter/adapter/index
   /autoapi/ommx_python_mip_adapter/exception/index
   /autoapi/ommx_python_mip_adapter/python_mip_to_ommx/index


Exceptions
----------

.. autoapisummary::

   ommx_python_mip_adapter.OMMXPythonMIPAdapterError


Classes
-------

.. autoapisummary::

   ommx_python_mip_adapter.OMMXPythonMIPAdapter


Functions
---------

.. autoapisummary::

   ommx_python_mip_adapter.model_to_instance


Package Contents
----------------

.. py:exception:: OMMXPythonMIPAdapterError



   Common base class for all non-exit exceptions.


.. py:class:: OMMXPythonMIPAdapter(ommx_instance: ommx.v1.Instance, *, relax: bool = False, solver_name: str = mip.CBC, solver: Optional[mip.Solver] = None, verbose: bool = False)



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


   .. py:method:: decode(data: mip.Model) -> ommx.v1.Solution

      Convert optimized Python-MIP model and ommx.v1.Instance to ommx.v1.Solution.

      This method is intended to be used if the model has been acquired with
      `solver_input` for futher adjustment of the solver parameters, and
      separately optimizing the model.

      Note that alterations to the model may make the decoding process
      incompatible -- decoding will only work if the model still describes
      effectively the same problem as the OMMX instance used to create the
      adapter.

      When creating the solution, this method reflects the `relax` flag used
      in this adapter's constructor. The solution's `relaxation` metadata will
      be set _only_ if `relax=True` was passed to the constructor. There is no
      way for this adapter to get relaxation information from Python-MIP
      directly. If relaxing the model separately after obtaining it with
      `solver_input`, you must set `solution.relaxation` yourself if you
      care about this value.

      Examples
      =========

      .. doctest::

          >>> from ommx.v1 import Instance, DecisionVariable
          >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter

          >>> p = [10, 13, 18, 32, 7, 15]
          >>> w = [11, 15, 20, 35, 10, 33]
          >>> x = [DecisionVariable.binary(i) for i in range(6)]
          >>> instance = Instance.from_components(
          ...     decision_variables=x,
          ...     objective=sum(p[i] * x[i] for i in range(6)),
          ...     constraints={0: sum(w[i] * x[i] for i in range(6)) <= 47},
          ...     sense=Instance.MAXIMIZE,
          ... )

          >>> adapter = OMMXPythonMIPAdapter(instance)
          >>> model = adapter.solver_input
          >>> # ... some modification of model's parameters
          >>> model.optimize()
          <OptimizationStatus.OPTIMAL: 0>

          >>> solution = adapter.decode(model)
          >>> solution.objective
          42.0




   .. py:method:: decode_to_state(data: mip.Model) -> ommx.v1.State

      Create an ommx.v1.State from an optimized Python-MIP Model.

      Examples
      =========

      .. doctest::

          The following example of solving an unconstrained linear optimization problem with x1 as the objective function.

          >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter
          >>> from ommx.v1 import Instance, DecisionVariable

          >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
          >>> ommx_instance = Instance.from_components(
          ...     decision_variables=[x1],
          ...     objective=x1,
          ...     constraints={},
          ...     sense=Instance.MINIMIZE,
          ... )
          >>> adapter = OMMXPythonMIPAdapter(ommx_instance)
          >>> model = adapter.solver_input
          >>> model.optimize()
          <OptimizationStatus.OPTIMAL: 0>

          >>> ommx_state = adapter.decode_to_state(model)
          >>> ommx_state.entries
          {1: 0.0}



   .. py:method:: solve(ommx_instance: ommx.v1.Instance, relax: bool = False, verbose: bool = False) -> ommx.v1.Solution
      :classmethod:


      Solve the given ommx.v1.Instance using Python-MIP, returning an ommx.v1.Solution.

      :param ommx_instance: The ommx.v1.Instance to solve.
      :param relax: If True, relax all integer variables to continuous variables by using the `relax` parameter in Python-MIP's `Model.optimize() <https://docs.python-mip.com/en/latest/classes.html#mip.Model.optimize>`.
      :param verbose: If True, enable Python-MIP's verbose mode

      Examples
      =========

      KnapSack Problem

      .. doctest::

          >>> from ommx.v1 import Instance, DecisionVariable
          >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter

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

          >>> solution = OMMXPythonMIPAdapter.solve(instance)

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
              >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter

              >>> x = DecisionVariable.integer(0, upper=3, lower=0)
              >>> instance = Instance.from_components(
              ...     decision_variables=[x],
              ...     objective=x,
              ...     constraints={0: x >= 4},
              ...     sense=Instance.MAXIMIZE,
              ... )

              >>> OMMXPythonMIPAdapter.solve(instance)
              Traceback (most recent call last):
                  ...
              ommx.adapter.InfeasibleDetected: Model was infeasible

      Unbounded Problem

      .. doctest::

              >>> from ommx.v1 import Instance, DecisionVariable
              >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter

              >>> x = DecisionVariable.integer(0, lower=0)
              >>> instance = Instance.from_components(
              ...     decision_variables=[x],
              ...     objective=x,
              ...     constraints={},
              ...     sense=Instance.MAXIMIZE,
              ... )

              >>> OMMXPythonMIPAdapter.solve(instance)
              Traceback (most recent call last):
                  ...
              ommx.adapter.UnboundedDetected: Model was unbounded

      Dual variable

      .. doctest::

              >>> from ommx.v1 import Instance, DecisionVariable
              >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter

              >>> x = DecisionVariable.continuous(0, lower=0, upper=1)
              >>> y = DecisionVariable.continuous(1, lower=0, upper=1)
              >>> instance = Instance.from_components(
              ...     decision_variables=[x, y],
              ...     objective=x + y,
              ...     constraints={0: x + y <= 1},
              ...     sense=Instance.MAXIMIZE,
              ... )

              >>> solution = OMMXPythonMIPAdapter.solve(instance)
              >>> solution.get_dual_variable(0)
              1.0




   .. py:attribute:: ADDITIONAL_CAPABILITIES
      :type:  frozenset[ommx.v1.AdditionalCapability]


   .. py:property:: solver_input
      :type: mip.Model


      The Python-MIP model generated from this OMMX instance



.. py:function:: model_to_instance(model: mip.Model) -> ommx.v1.Instance

   The function to convert Python-MIP Model to ommx.v1.Instance.

   Examples
   =========

   .. doctest::
       >>> import mip
       >>> import ommx_python_mip_adapter as adapter

       >>> model = mip.Model()
       >>> x1=model.add_var(name="1", var_type=mip.INTEGER, lb=0, ub=5)
       >>> x2=model.add_var(name="2", var_type=mip.CONTINUOUS, lb=0, ub=5)

       >>> model.objective = - x1 - 2 * x2
       >>> constr = model.add_constr(x1 + x2 - 6 <= 0)

       >>> ommx_instance = adapter.model_to_instance(model)


