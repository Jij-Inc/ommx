---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
---

# About Advanced Examples

Advanced examples are longer, practical examples that combine OMMX modeling,
adapter orchestration, decomposition methods, and solver-specific workflows.

Unlike tutorials, these pages are allowed to spend more space on design tradeoffs,
intermediate data structures, and reusable implementation patterns. The goal is to
show how to build real optimization workflows on top of `ommx.v1.Instance`,
`ommx.v1.Solution`, and OMMX adapters.

The examples in this section may use multiple adapters together. When an example
uses an adapter-specific feature, it keeps that dependency at the workflow boundary
so that the underlying OMMX model remains reusable.
