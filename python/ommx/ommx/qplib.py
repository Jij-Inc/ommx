from . import Instance, State


def load_file(path: str) -> Instance:
    return Instance.load_qplib(path)


def load_solution(path: str, *, num_variables: int) -> State:
    """
    Load candidate values using :meth:`ommx.State.load_qplib_solution`.

    **Evaluate a published candidate**

    Download the `problem <https://qplib.zib.de/qplib/QPLIB_0018.qplib>`_ and
    `candidate <https://qplib.zib.de/sol/QPLIB_0018.sol>`_ for QPLIB 0018
    into your working directory. The problem defines the variables, their
    domains, the constraints, and the objective to minimize. The candidate
    assigns values to those variables. Use a matching pair of files so that
    the variable IDs in the assignment refer to the intended model.
    QPLIB credits Andrea Scozzari for this instance and distributes it under
    `CC BY 4.0 <https://creativecommons.org/licenses/by/4.0/>`_.

    >>> from ommx import ConstraintKind, Instance
    >>> from ommx.qplib import load_solution
    >>> instance = Instance.load_qplib("QPLIB_0018.qplib")
    >>> candidate = load_solution("QPLIB_0018.sol", num_variables=len(instance.decision_variables))
    >>> solution = instance.evaluate(candidate, atol=1e-8)
    >>> solution.feasible, round(solution.objective, 6)
    (True, -6.386015)

    Passing the variable count fills omitted coordinates with zero. Evaluation
    computes the objective from the model and checks variable domains and
    constraints at the given tolerance. The objective in the file is not an
    assignment to a variable or a proof of optimality. QPLIB 0018 minimizes
    its objective, so a smaller value is better among feasible candidates.

    >>> variables = solution.decision_variables_df()
    >>> selected = variables.loc[variables["value"] != 0, ["lower", "value", "upper"]]
    >>> assert (selected["lower"] <= selected["value"]).all()

    To see which constraint a different candidate violates, assign a value
    of zero for every variable. This problem requires their sum to be one.

    >>> zeros = {v.id: 0.0 for v in instance.decision_variables}
    >>> rejected = instance.evaluate(zeros, atol=1e-8)
    >>> rejected.feasible, rejected.objective
    (False, 0.0)
    >>> rows = rejected.constraints_df(kind=ConstraintKind.Regular)
    >>> rows.loc[~rows["feasible"], "violation"].tolist()
    [1.0]

    Filter on the feasibility flag to respect the numerical tolerance.
    :meth:`ommx.Solution.decision_variables_df` shows the values and bounds
    for individual variables. If you also want to try a candidate as a
    starting point, see :meth:`ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter.solve`.
    """
    return State.load_qplib_solution(path, num_variables=num_variables)
