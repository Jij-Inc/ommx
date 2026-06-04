ommx.adapter
============

.. py:module:: ommx.adapter


Attributes
----------

.. autoapisummary::

   ommx.adapter.SamplerInput
   ommx.adapter.SamplerOutput
   ommx.adapter.SolverInput
   ommx.adapter.SolverOutput


Exceptions
----------

.. autoapisummary::

   ommx.adapter.InfeasibleDetected
   ommx.adapter.NoSolutionReturned
   ommx.adapter.UnboundedDetected


Classes
-------

.. autoapisummary::

   ommx.adapter.SamplerAdapter
   ommx.adapter.SolverAdapter


Module Contents
---------------

.. py:exception:: InfeasibleDetected



   Raised when the problem is proven to be infeasible.

   This corresponds to ``Optimality.OPTIMALITY_INFEASIBLE`` and indicates that
   the mathematical model itself has no feasible solution.
   Should not be used when infeasibility cannot be proven (e.g., heuristic solvers).


.. py:exception:: NoSolutionReturned



   Raised when no solution was returned.

   This indicates that the solver did not return any solution (whether feasible
   or not) (e.g., due to time limits).
   This does not prove that the mathematical model itself is infeasible.


.. py:exception:: UnboundedDetected



   Raised when the problem is proven to be unbounded.

   This corresponds to ``Optimality.OPTIMALITY_UNBOUNDED`` and indicates that
   the mathematical model itself is unbounded.
   Should not be used when unboundedness cannot be proven (e.g., heuristic solvers).


.. py:class:: SamplerAdapter(ommx_instance: ommx.v1.Instance)



   An abstract interface for OMMX Sampler Adapters, defining how samplers should be used with OMMX.

   See the `implementation guide <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_ for more details.


   .. py:method:: decode(data: SolverOutput) -> ommx.v1.Solution
      :abstractmethod:



   .. py:method:: decode_to_sampleset(data: SamplerOutput) -> ommx.v1.SampleSet
      :abstractmethod:



   .. py:method:: sample(ommx_instance: ommx.v1.Instance) -> ommx.v1.SampleSet
      :classmethod:

      :abstractmethod:



   .. py:method:: solve(ommx_instance: ommx.v1.Instance) -> ommx.v1.Solution
      :classmethod:

      :abstractmethod:



   .. py:attribute:: ADDITIONAL_CAPABILITIES
      :type:  frozenset[ommx.v1.AdditionalCapability]


   .. py:property:: sampler_input
      :type: SamplerInput

      :abstractmethod:



   .. py:property:: solver_input
      :type: SolverInput

      :abstractmethod:



.. py:class:: SolverAdapter(ommx_instance: ommx.v1.Instance)



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


   .. py:method:: decode(data: SolverOutput) -> ommx.v1.Solution
      :abstractmethod:



   .. py:method:: solve(ommx_instance: ommx.v1.Instance) -> ommx.v1.Solution
      :classmethod:

      :abstractmethod:



   .. py:attribute:: ADDITIONAL_CAPABILITIES
      :type:  frozenset[ommx.v1.AdditionalCapability]


   .. py:property:: solver_input
      :type: SolverInput

      :abstractmethod:



.. py:data:: SamplerInput

.. py:data:: SamplerOutput

.. py:data:: SolverInput

.. py:data:: SolverOutput

