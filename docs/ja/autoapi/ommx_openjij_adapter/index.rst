ommx_openjij_adapter
====================

.. py:module:: ommx_openjij_adapter


Classes
-------

.. autoapisummary::

   ommx_openjij_adapter.OMMXOpenJijSAAdapter


Functions
---------

.. autoapisummary::

   ommx_openjij_adapter.decode_to_samples
   ommx_openjij_adapter.response_to_samples
   ommx_openjij_adapter.sample_qubo_sa


Package Contents
----------------

.. py:class:: OMMXOpenJijSAAdapter(ommx_instance: ommx.v1.Instance, *, beta_min: float | None = None, beta_max: float | None = None, num_sweeps: int | None = None, num_reads: int | None = None, schedule: list | None = None, initial_state: list | dict | None = None, updater: str | None = None, sparse: bool | None = None, reinitialize_state: bool | None = None, seed: int | None = None, uniform_penalty_weight: Optional[float] = None, penalty_weights: dict[int, float] = {}, inequality_integer_slack_max_range: int = 32)



   Sampling QUBO or HUBO with Simulated Annealing (SA) by `openjij.SASampler <https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SASampler>`_


   .. py:method:: decode(data: openjij.Response) -> ommx.v1.Solution


   .. py:method:: decode_to_samples(data: openjij.Response) -> ommx.v1.Samples

      Convert `openjij.Response <https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.Response>`_ to :class:`Samples`

      There is a static method :meth:`decode_to_samples` that does the same thing.



   .. py:method:: decode_to_sampleset(data: openjij.Response) -> ommx.v1.SampleSet


   .. py:method:: sample(ommx_instance: ommx.v1.Instance, *, beta_min: float | None = None, beta_max: float | None = None, num_sweeps: int | None = None, num_reads: int | None = None, schedule: list | None = None, initial_state: list | dict | None = None, updater: str | None = None, sparse: bool | None = None, reinitialize_state: bool | None = None, seed: int | None = None, uniform_penalty_weight: Optional[float] = None, penalty_weights: dict[int, float] = {}, inequality_integer_slack_max_range: int = 32) -> ommx.v1.SampleSet
      :classmethod:



   .. py:method:: solve(ommx_instance: ommx.v1.Instance, *, beta_min: float | None = None, beta_max: float | None = None, num_sweeps: int | None = None, num_reads: int | None = None, schedule: list | None = None, initial_state: list | dict | None = None, updater: str | None = None, sparse: bool | None = None, reinitialize_state: bool | None = None, seed: int | None = None, uniform_penalty_weight: Optional[float] = None, penalty_weights: dict[int, float] = {}, inequality_integer_slack_max_range: int = 32) -> ommx.v1.Solution
      :classmethod:



   .. py:attribute:: ADDITIONAL_CAPABILITIES
      :type:  frozenset[ommx.v1.AdditionalCapability]


   .. py:attribute:: beta_max
      :type:  float | None
      :value: None


      maximum value of inverse temperature 



   .. py:attribute:: beta_min
      :type:  float | None
      :value: None


      minimal value of inverse temperature 



   .. py:attribute:: inequality_integer_slack_max_range
      :type:  int
      :value: 32


      Max range for integer slack variables in inequality constraints, passed to ``Instance.to_qubo`` or ``Instance.to_hubo`` 



   .. py:attribute:: initial_state
      :type:  list | dict | None
      :value: None


      initial state (parameter only used if problem is QUBO)



   .. py:attribute:: num_reads
      :type:  int | None
      :value: None


      number of reads 



   .. py:attribute:: num_sweeps
      :type:  int | None
      :value: None


      number of sweeps 



   .. py:attribute:: ommx_instance
      :type:  ommx.v1.Instance

      ommx.v1.Instance representing a QUBO or HUBO problem

      The input `instance` must be a QUBO (Quadratic Unconstrained Binary Optimization) or HUBO (Higher-order Unconstrained Binary Optimization) problem, i.e.

      - All decision variables are binary
      - No constraints
      - Objective function is quadratic (QUBO) or higher (HUBO).
      - Minimization problem

      You can convert an instance to QUBO or HUBO via :meth:`ommx.v1.Instance.penalty_method` or other corresponding method.



   .. py:attribute:: penalty_weights
      :type:  dict[int, float]

      Penalty weights for each constraint, passed to ``Instance.to_qubo`` or ``Instance.to_hubo`` 



   .. py:attribute:: reinitialize_state
      :type:  bool | None
      :value: None


      if true reinitialize state for each run (parameter only used if problem is QUBO)



   .. py:property:: sampler_input
      :type: dict[tuple[int, Ellipsis], float]



   .. py:attribute:: schedule
      :type:  list | None
      :value: None


      list of inverse temperature (parameter only used if problem is QUBO)



   .. py:attribute:: seed
      :type:  int | None
      :value: None


      seed for Monte Carlo algorithm 



   .. py:property:: solver_input
      :type: dict[tuple[int, Ellipsis], float]



   .. py:attribute:: sparse
      :type:  bool | None
      :value: None


      use sparse matrix or not (parameter only used if problem is QUBO)



   .. py:attribute:: uniform_penalty_weight
      :type:  Optional[float]
      :value: None


      Weight for uniform penalty, passed to ``Instance.to_qubo`` or ``Instance.to_hubo`` 



   .. py:attribute:: updater
      :type:  str | None
      :value: None


      updater algorithm 



.. py:function:: decode_to_samples(response: openjij.Response) -> ommx.v1.Samples

   Convert `openjij.Response <https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.Response>`_ to :class:`Samples`


.. py:function:: response_to_samples(response: openjij.Response) -> ommx.v1.Samples

   Deprecated: renamed to :meth:`decode_to_samples`


.. py:function:: sample_qubo_sa(instance: ommx.v1.Instance, *, beta_min: float | None = None, beta_max: float | None = None, num_sweeps: int | None = None, num_reads: int | None = None, schedule: list | None = None, initial_state: list | dict | None = None, updater: str | None = None, sparse: bool | None = None, reinitialize_state: bool | None = None, seed: int | None = None) -> ommx.v1.Samples

   Deprecated: Use :meth:`OMMXOpenJijSAAdapter.sample` instead


