ommx.testing.placement
======================

.. py:module:: ommx.testing.placement

.. autoapi-nested-parse::

   Plant Placement Problem — equivalent OMMX formulations.

   This module provides a small, solver-agnostic benchmark problem used to
   exercise an adapter's SOS1 handling. The builders in this module produce
   `ommx.v1.Instance` objects describing the same feasible region and optimum;
   they differ only in how "at most one plant per region" is communicated to
   the solver.

   Problem
   -------

   A set of plants and clients are drawn uniformly from :math:`[0, 100]^2`. A
   vertical line at :math:`x = 50` partitions the plants into a *west* region
   :math:`W = \{i : x_i^{\text{plant}} < 50\}` and an *east* region
   :math:`E = \{i : x_i^{\text{plant}} \ge 50\}`. At most one plant may be
   opened in each region; the opened plant covers all of its region's share of
   client demand via a continuous transport variable.

   Sets and parameters
   ~~~~~~~~~~~~~~~~~~~

   - :math:`N` plants, :math:`M` clients.
   - :math:`C_i \ge 0` — maximum capacity of plant :math:`i`.
   - :math:`d_j \ge 0` — demand of client :math:`j`.
   - :math:`\operatorname{dist}(i, j)` — Euclidean distance between plant
     :math:`i` and client :math:`j`.

   Decision variables (shared by both formulations)
   ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

   .. math::

       \begin{aligned}
       s_{i,j} &\in [0, d_j] \qquad i \in 1..N,\ j \in 1..M \\
       c_i     &\in [0, C_i] \qquad i \in 1..N
       \end{aligned}

   where :math:`s_{i,j}` is the amount delivered from plant :math:`i` to client
   :math:`j` and :math:`c_i` is the total capacity drawn from plant :math:`i`.

   Shared constraints
   ~~~~~~~~~~~~~~~~~~

   .. math::

       \begin{aligned}
       \sum_{j=1}^M s_{i,j} &= c_i \quad \text{(capacity balance, per plant)} \\
       \sum_{i=1}^N s_{i,j} &= d_j \quad \text{(demand, per client)}
       \end{aligned}

   Objective (minimize)
   ~~~~~~~~~~~~~~~~~~~~

   .. math::

       \min \; \sum_{i,j} \operatorname{dist}(i, j) \cdot s_{i,j}
             \;+\; \sum_i c_i

   "At most one plant per region" — eight formulations
   ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

   All eight builders share the decision variables and constraints above and
   encode the same feasible region on :math:`(s, c)`. They differ along three
   orthogonal axes:

   - whether the auxiliary opening indicator :math:`\delta_i \in \{0, 1\}` and
     the big-M link :math:`c_i \le C_i \, \delta_i` are introduced;
   - whether the per-region cardinality :math:`\sum_{i \in W} \delta_i \le 1`
     (and the analogous east bound) is added as a plain linear constraint;
   - where SOS1 is declared: on the continuous capacities
     :math:`\{c_i\}_{i \in W/E}`, on the binary indicators
     :math:`\{\delta_i\}_{i \in W/E}`, on **both** (redundant), or nowhere.

   The eight builders enumerate every well-defined combination:

   +---+-----------------------------------------+------------+---------------------+--------------+--------------+
   | # | Builder                                 | δ + big-M  | :math:`\sum δ ≤ 1`  | SOS1 on `c`  | SOS1 on `δ`  |
   +===+=========================================+============+=====================+==============+==============+
   | 1 | :func:`build_sos1`                      | –          | –                   | ✓            | –            |
   +---+-----------------------------------------+------------+---------------------+--------------+--------------+
   | 2 | :func:`build_sos1_on_c_with_delta`      | ✓          | –                   | ✓            | –            |
   +---+-----------------------------------------+------------+---------------------+--------------+--------------+
   | 3 | :func:`build_sos1_on_c_with_delta_with_card` | ✓     | ✓                   | ✓            | –            |
   +---+-----------------------------------------+------------+---------------------+--------------+--------------+
   | 4 | :func:`build_sos1_on_delta`             | ✓          | –                   | –            | ✓            |
   +---+-----------------------------------------+------------+---------------------+--------------+--------------+
   | 5 | :func:`build_sos1_on_delta_with_card`   | ✓          | ✓                   | –            | ✓            |
   +---+-----------------------------------------+------------+---------------------+--------------+--------------+
   | 6 | :func:`build_sos1_on_both_with_delta`   | ✓          | –                   | ✓            | ✓            |
   +---+-----------------------------------------+------------+---------------------+--------------+--------------+
   | 7 | :func:`build_sos1_on_both_with_delta_with_card` | ✓  | ✓                   | ✓            | ✓            |
   +---+-----------------------------------------+------------+---------------------+--------------+--------------+
   | 8 | :func:`build_bigm`                      | ✓          | ✓                   | –            | –            |
   +---+-----------------------------------------+------------+---------------------+--------------+--------------+

   The δ-bearing rows always include the big-M link
   :math:`c_i \le C_i \, \delta_i`. Each region with at least two plants
   contributes one SOS1 set per "✓" in the SOS1 columns; regions with fewer
   than two plants are skipped (the constraint is trivially satisfied).

   Intended use
   ------------

   These eight builders are useful for benchmarking how a solver — and the
   adapter forwarding to it — reacts to different ways of expressing the same
   SOS1 structure. Callers should construct :meth:`Input.random` with a fixed
   ``random.seed`` for reproducibility and pass the resulting :class:`Input`
   to each builder to obtain comparable instances.



Classes
-------

.. autoapisummary::

   ommx.testing.placement.Client
   ommx.testing.placement.Input
   ommx.testing.placement.Plant


Functions
---------

.. autoapisummary::

   ommx.testing.placement.build_bigm
   ommx.testing.placement.build_sos1
   ommx.testing.placement.build_sos1_on_both_with_delta
   ommx.testing.placement.build_sos1_on_both_with_delta_with_card
   ommx.testing.placement.build_sos1_on_c_with_delta
   ommx.testing.placement.build_sos1_on_c_with_delta_with_card
   ommx.testing.placement.build_sos1_on_delta
   ommx.testing.placement.build_sos1_on_delta_with_card


Module Contents
---------------

.. py:class:: Client

   .. py:attribute:: demand
      :type:  float


   .. py:attribute:: position
      :type:  Tuple[float, float]


.. py:class:: Input

   .. py:method:: random(num_plants: int, num_clients: int) -> Input
      :classmethod:


      Sample a random instance that is feasible under the one-plant-per-region rule.

      Plant and client positions are drawn uniformly from :math:`[0, 100]^2`.
      Client demands are uniform on :math:`[200, 400]`. Plant capacities are
      drawn from a range sized relative to total demand.

      To keep the sampled instance feasible under "at most one plant per
      region", two repairs are applied:

      1. Plant positions are resampled until both west and east regions
         contain at least one plant.
      2. If the *best* plant in each region together cannot cover total
         demand, the deficit is split evenly between those two best plants.
         Only the two best plants are touched — all other capacities are
         left at their sampled values, so the benchmark difficulty is not
         inflated across the board.

      Requires ``num_plants >= 2``. Callers are expected to seed
      :mod:`random` before calling this.



   .. py:attribute:: clients
      :type:  List[Client]


   .. py:attribute:: plants
      :type:  List[Plant]


.. py:class:: Plant

   .. py:attribute:: max_capacity
      :type:  float


   .. py:attribute:: position
      :type:  Tuple[float, float]


.. py:function:: build_bigm(input: Input) -> ommx.v1.Instance

   Pure linear: big-M link plus per-region cardinality bounds, no SOS1.


.. py:function:: build_sos1(input: Input) -> ommx.v1.Instance

   Build the instance with one SOS1 constraint per region on :math:`c_i`.

   A region with fewer than two plants is trivially satisfied and is skipped.


.. py:function:: build_sos1_on_both_with_delta(input: Input) -> ommx.v1.Instance

   δ + big-M; SOS1 declared on both c and δ (no explicit cardinality).


.. py:function:: build_sos1_on_both_with_delta_with_card(input: Input) -> ommx.v1.Instance

   δ + big-M + cardinality; SOS1 declared on both c and δ (maximum-information).


.. py:function:: build_sos1_on_c_with_delta(input: Input) -> ommx.v1.Instance

   δ + big-M; per-region cardinality enforced by SOS1 on the continuous c_i.


.. py:function:: build_sos1_on_c_with_delta_with_card(input: Input) -> ommx.v1.Instance

   δ + big-M + cardinality AND a SOS1 on the continuous c_i (cardinality kept).


.. py:function:: build_sos1_on_delta(input: Input) -> ommx.v1.Instance

   δ + big-M; per-region cardinality replaced by SOS1 on the binaries.


.. py:function:: build_sos1_on_delta_with_card(input: Input) -> ommx.v1.Instance

   δ + big-M + cardinality bounds AND a redundant SOS1 on the binaries.


