# ommx-column-generation

Experimental column generation helpers for OMMX.

This package starts as a working MVP: build a restricted master problem from rows
and columns, solve it through a callback, pass dual values to a pricing oracle, and
append generated columns.

The API is intentionally small and unstable while the decomposition data model is
being explored.
