# ommx-column-generation

Experimental column generation helpers for OMMX.

This package starts as a working MVP: build a restricted master problem from rows
and columns, solve it through a callback, pass dual values to a pricing oracle, and
append generated columns.

The API is intentionally small and unstable while the decomposition data model is
being explored.

## Example

Run a tiny covering example:

```bash
python examples/covering.py
```

The initial RMP has two single-cover columns, `a` and `b`.  The pricing oracle
scans a finite catalog, finds the combined column `ab` by negative reduced cost,
adds it to the RMP, and resolves the LP.
