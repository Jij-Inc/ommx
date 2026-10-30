Adapter packages and input contracts
====================================

An Adapter connects an OMMX Instance to a solver or sampler and returns an
evaluated :class:`~ommx.Solution` or :class:`~ommx.SampleSet`. The return type
does not determine the search method or its optimality guarantees. See
:class:`~ommx.adapter.SolverAdapter` and :class:`~ommx.adapter.SamplerAdapter`
for the common contract.

Inspect the concrete Adapter's ``INPUT_CLASS`` for the exact input it accepts.
The recommended preparation policy is an editable starting point; constructing
it neither changes an Instance nor guarantees that a particular problem can be
prepared. The easy ``solve`` and ``sample`` paths prepare an isolated copy.
Constructors and ``*_without_preparation`` paths require an already applicable
input. The :class:`~ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter` Reference has a
complete example of selecting a policy, preparing a model, and solving it
without further preparation.

Packages developed in this repository
-------------------------------------

Until the 3.0.0 stable release, this Reference and the Book both describe
``main``. Follow the :doc:`installation guide </getting_started/install>` to
install the SDK and adapters from the same checkout.

.. list-table::
   :header-rows: 1

   * - Package
     - Backend
     - Input and execution contract
   * - `ommx-pyscipopt-adapter <https://pypi.org/project/ommx-pyscipopt-adapter/>`_
     - SCIP / PySCIPOpt
     - :class:`~ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter`
   * - `ommx-highs-adapter <https://pypi.org/project/ommx-highs-adapter/>`_
     - HiGHS
     - :class:`~ommx_highs_adapter.OMMXHighsAdapter`
   * - `ommx-openjij-adapter <https://pypi.org/project/ommx-openjij-adapter/>`_
     - OpenJij SA / SQA
     - :mod:`~ommx_openjij_adapter`
   * - `ommx-python-mip-adapter <https://pypi.org/project/ommx-python-mip-adapter/>`_
     - Python-MIP
     - :class:`~ommx_python_mip_adapter.OMMXPythonMIPAdapter`

A backend name is not an input class. For example,
:class:`~ommx_openjij_adapter.OMMXOpenJijSAAdapter` accepts unconstrained binary
polynomial minimization of arbitrary polynomial degree; it is not restricted
to QUBO. Use its declared ``INPUT_CLASS`` and recommended policy rather than
assuming that every OpenJij model must first become quadratic.

Settings such as penalty weights, tolerances, and time or sample limits remain
application choices. Diagnostic and initial-state support also differ by
Adapter; consult the concrete package Reference.

Integrations maintained in separate repositories
------------------------------------------------

The following projects maintain their own SDK compatibility, execution
environment, authentication or license requirements, and result contracts.
Consult the selected project's release documentation for those conditions.

.. list-table::
   :header-rows: 1

   * - Project
     - Package
   * - `Gurobi <https://github.com/Jij-Inc/ommx-gurobipy-adapter>`_
     - `ommx-gurobipy-adapter <https://pypi.org/project/ommx-gurobipy-adapter/>`_
   * - `D-Wave <https://github.com/Jij-Inc/ommx-dwave-adapter>`_
     - `ommx-dwave-adapter <https://pypi.org/project/ommx-dwave-adapter/>`_
   * - `Fixstars Amplify <https://github.com/Jij-Inc/ommx-fixstars-amplify-adapter>`_
     - `ommx-fixstars-amplify-adapter <https://pypi.org/project/ommx-fixstars-amplify-adapter/>`_
   * - `Fujitsu Digital Annealer <https://github.com/Jij-Inc/ommx-da4-adapter>`_
     - `ommx-da4-adapter <https://pypi.org/project/ommx-da4-adapter/>`_
   * - `Kipu Iskay <https://github.com/Jij-Inc/ommx-kipu-iskay-adapter>`_
     - `ommx-kipu-iskay-adapter <https://pypi.org/project/ommx-kipu-iskay-adapter/>`_
   * - `Q-CTRL QAOA <https://github.com/Jij-Inc/ommx-qctrl-qaoa-adapter>`_
     - `ommx-qctrl-qaoa-adapter <https://pypi.org/project/ommx-qctrl-qaoa-adapter/>`_
