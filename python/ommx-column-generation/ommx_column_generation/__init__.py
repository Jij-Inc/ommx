r"""Minimal column generation primitives for OMMX.

This package provides the small core needed to run a column generation loop.
It deliberately does not require the pricing problem to be represented as an
OMMX :class:`~ommx.v1.ParametricInstance`.  A
:class:`~ommx_column_generation.PricingOracle` can be implemented with an OMMX
model, a dynamic program, a shortest-path solver, an annealer, or any other
problem-specific method.

Mathematical Form
=================

The current API works with a restricted master problem (RMP) written in column
space.  Given a finite working set of columns :math:`J'`, the RMP has one
variable :math:`\lambda_j` for each column:

.. math::

   \begin{array}{ll}
   \min & c_0 + \sum_{j \in J'} c_j \lambda_j \\
   \textrm{s.t.}
        & \sum_{j \in J'} a_{ij} \lambda_j \ \bowtie_i \ b_i,
          \quad i \in I, \\
        & \lambda_j \ge 0, \quad j \in J'.
   \end{array}

Here :math:`i` is a master row, :math:`j` is a column,
:math:`c_j` is the column cost, :math:`a_{ij}` is the row activity of the
column, and :math:`\bowtie_i` is one of :math:`\le`, :math:`\ge`, or
:math:`=`.

After solving the RMP, the dual value :math:`\pi_i` of each master row is
passed to a pricing oracle.  In a minimization problem, an exact pricing oracle
typically searches for a column with negative reduced cost:

.. math::

   \bar{c}(x) = c(x) - \sum_{i \in I} \pi_i a_i(x).

If no column with :math:`\bar{c}(x) < 0` exists, the current RMP solution is
optimal for the LP relaxation represented by the pricing oracle.  Convexity
constraints, block constraints, and other decomposition-specific terms can be
represented as additional master rows or handled inside the pricing oracle.
The current loop is an LP column generation loop: it solves continuous RMPs to
obtain dual values.  If an integer solution is needed, the generated column
pool can be solved once as a restricted integer master problem after the loop;
this is not branch-and-price.

Example
=======

The following tiny covering problem starts with two single-cover columns.  The
pricing oracle scans a finite catalog, finds a combined column with negative
reduced cost, and adds it to the RMP.

.. doctest::

   >>> from ommx_column_generation import (
   ...     Column,
   ...     ColumnGenerationProblem,
   ...     MasterRow,
   ...     PricingResult,
   ...     highs_master_solver,
   ...     solve_column_generation,
   ... )
   >>> problem = ColumnGenerationProblem(
   ...     rows=[
   ...         MasterRow("cover_a", ">=", 1.0),
   ...         MasterRow("cover_b", ">=", 1.0),
   ...     ],
   ...     columns=[
   ...         Column("a", 2.0, {"cover_a": 1.0}),
   ...         Column("b", 2.0, {"cover_b": 1.0}),
   ...     ],
   ... )
   >>> catalog = [
   ...     Column("ab", 3.0, {"cover_a": 1.0, "cover_b": 1.0}),
   ... ]
   >>> def pricing_oracle(context):
   ...     existing_ids = {column.id for column in context.columns}
   ...     improving_columns = []
   ...     for column in catalog:
   ...         if column.id in existing_ids:
   ...             continue
   ...         reduced_cost = column.cost - sum(
   ...             context.duals[row_id] * coefficient
   ...             for row_id, coefficient in column.coefficients.items()
   ...         )
   ...         if reduced_cost < -context.tolerance:
   ...             improving_columns.append(column)
   ...     return PricingResult(
   ...         improving_columns,
   ...         proven_no_negative_reduced_cost=not improving_columns,
   ...     )
   >>> result = solve_column_generation(
   ...     problem,
   ...     master_solver=highs_master_solver,
   ...     pricing_oracle=pricing_oracle,
   ... )
   >>> result.master_solution.objective
   3.0
   >>> result.column_values
   {'a': 0.0, 'b': 0.0, 'ab': 1.0}
   >>> [record.accepted_column_ids for record in result.iterations]
   [('ab',), ()]

API Roles
=========

:class:`~ommx_column_generation.MasterRow`
    Defines one row :math:`i \in I` of the RMP: its stable row ID, sense, and
    right-hand side :math:`b_i`.

:class:`~ommx_column_generation.Column`
    Defines one generated column :math:`j`: its cost :math:`c_j` and row
    coefficients :math:`a_{ij}` keyed by
    :attr:`~ommx_column_generation.MasterRow.id`.  The optional
    :attr:`~ommx_column_generation.Column.payload` stores problem-specific data
    such as the original subproblem solution.

:class:`~ommx_column_generation.ColumnGenerationProblem`
    Holds the current working set of rows and columns.  Its
    :meth:`~ommx_column_generation.ColumnGenerationProblem.build_restricted_master`
    method builds the current RMP as an :class:`~ommx.v1.Instance`.

:class:`~ommx_column_generation.RestrictedMasterProblem`
    Wraps the generated RMP instance and the ID mappings needed to read
    :math:`\lambda_j` values and dual values back from an OMMX
    :class:`~ommx.v1.Solution`.

:class:`~ommx_column_generation.PricingContext` and :class:`~ommx_column_generation.PricingResult`
    Define the contract between the RMP loop and the pricing oracle.  The
    context contains the current solution and duals; the result returns newly
    generated columns and whether optimality of the pricing step was proven.

:class:`~ommx_column_generation.PricingOracle`
    The problem-specific pricing boundary.  It receives duals and returns
    columns.  This can internally solve an OMMX
    :class:`~ommx.v1.ParametricInstance` or use a completely different
    algorithm.

:func:`~ommx_column_generation.solve_column_generation`
    Runs the loop: solve RMP, extract duals, call pricing, append accepted
    columns, and repeat until pricing returns no accepted columns or the
    iteration limit is reached.
"""

from .core import (
    Column,
    ColumnGenerationProblem,
    ColumnGenerationResult,
    IterationRecord,
    MasterRow,
    PricingContext,
    PricingOracle,
    PricingResult,
    RestrictedMasterProblem,
    RowSense,
    solve_column_generation,
)
from .solvers import highs_master_solver

__all__ = [
    "Column",
    "ColumnGenerationProblem",
    "ColumnGenerationResult",
    "IterationRecord",
    "MasterRow",
    "PricingContext",
    "PricingOracle",
    "PricingResult",
    "RestrictedMasterProblem",
    "RowSense",
    "highs_master_solver",
    "solve_column_generation",
]
