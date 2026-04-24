# Tutorial

*This tutorial is a work in progress.*

This document will walk through the `ommx` crate with runnable examples covering:

- Building expressions with [`linear!`](../macro.linear.html),
  [`quadratic!`](../macro.quadratic.html), and [`coeff!`](../macro.coeff.html)
- Defining decision variables, constraints, and an
  [`Instance`](../struct.Instance.html)
- Evaluating an instance against a
  [`State`](../v1/struct.State.html) to produce a
  [`Solution`](../struct.Solution.html)
- Interchanging problems via MPS / QPLIB / OMMX Artifact

For a high-level overview of the public API, see the
[crate-level documentation](../index.html).
