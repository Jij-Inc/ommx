r"""Minimal column generation primitives for OMMX.

This package provides the small core needed to run a column generation loop.
It deliberately does not require the pricing problem to be represented as an
OMMX ``ParametricInstance``.  A pricing oracle can be implemented with an OMMX
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

API Roles
=========

``MasterRow``
    Defines one row :math:`i \in I` of the RMP: its stable row ID, sense, and
    right-hand side :math:`b_i`.

``Column``
    Defines one generated column :math:`j`: its cost :math:`c_j` and row
    coefficients :math:`a_{ij}` keyed by ``MasterRow.id``.  The optional
    ``payload`` stores problem-specific data such as the original subproblem
    solution.

``ColumnGenerationProblem``
    Holds the current working set of rows and columns.  Its
    ``build_restricted_master`` method builds the current RMP as an
    ``ommx.v1.Instance``.

``RestrictedMasterProblem``
    Wraps the generated RMP instance and the ID mappings needed to read
    :math:`\lambda_j` values and dual values back from an OMMX ``Solution``.

``PricingContext`` and ``PricingResult``
    Define the contract between the RMP loop and the pricing oracle.  The
    context contains the current solution and duals; the result returns newly
    generated columns and whether optimality of the pricing step was proven.

``PricingOracle``
    The problem-specific pricing boundary.  It receives duals and returns
    columns.  This can internally solve an OMMX ``ParametricInstance`` or use a
    completely different algorithm.

``solve_column_generation``
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
