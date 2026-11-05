# OMMX

[![Docs/en](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/)
[![Docs/ja](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/)
[![Discord-EN](https://img.shields.io/badge/Discord-English-default?logo=Discord)](https://discord.gg/QPMbdJCbAB)
[![Discord-JP](https://img.shields.io/badge/Discord-日本語-default?logo=Discord)](https://discord.gg/CPd9ZsxtR5)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/Jij-Inc/ommx)

OMMX (Open Mathematical prograMming eXchange) lets you describe **what you want to solve** separately from **how you solve it**. Its Python and Rust SDKs provide a common representation for optimization problems and candidate solutions. You can change the formulation you send to a solver and still evaluate the result against the domains, constraints, and objective of your original problem.

## Why use OMMX?

Suppose a production line can make up to 100 units per hour, but only 60 in reduced mode. You can record the conditional limit once as an **indicator constraint**. The PySCIPOpt adapter can pass this constraint directly to SCIP. To try a different formulation with the same solver, you can instead use OMMX to convert the rule to a linear inequality using the known upper bound of 100.

Either way, a candidate that produces 65 units per hour in reduced mode violates the original rule. OMMX can check that rule for a candidate from either formulation, or even one proposed by a planner. You can try a new formulation without rewriting the requirement or the check. Checking feasibility tells you whether the candidate satisfies the model; proving optimality requires a separate guarantee.

The same division of work applies to other tasks:

- **Prepare a problem for a solver or sampler.** Reuse supported operations such as integer encoding and bound tightening. You choose which transformations are appropriate and, when constraints become penalties, how to weight them. Adapters handle backend input and translate results back into OMMX.
- **Compare candidates on the original problem.** Recover fixed or dependent variable values when a transformation records the information needed to do so. Evaluate candidates against the same constraints and objective regardless of where they came from.
- **Save what you need for later.** Artifacts package problems and results with metadata. Experiments record trials and settings when you want to revisit a comparison.

Today, OMMX provides individual transformations and preparation policies. The longer-term goal is to use the structure of a problem to select and combine preprocessing steps. Developing these techniques on a common representation lets improvements benefit multiple modeling tools and gives new solving methods a body of work to build on.

## Find your next step

The **Book** explains when to use each capability and how to interpret the results. The **API Reference** documents individual operations, their conditions, and executable usage examples.

| Purpose | Start here |
|:--------|:-----------|
| Understand OMMX and explore its capabilities | [English Book](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/) · [日本語Book](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/) |
| Choose how to represent a problem for a solver | [Transform a problem](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/user_guide/transform_and_solve.html) · [日本語](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/user_guide/transform_and_solve.html) |
| Check a candidate against your requirements | [Evaluate candidates](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/user_guide/evaluate_and_interpret.html) · [日本語](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/user_guide/evaluate_and_interpret.html) |
| Choose a solver or sampler integration | [Adapter packages and input contracts](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/api/adapters.html) |
| Install the Python SDK and an optional adapter | [Installation guide](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/getting_started/install.html) · [日本語](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/getting_started/install.html) |
| Inspect and exchange saved data from the command line | [OMMX CLI](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/user_guide/cli.html) |

Until the 3.0.0 stable release, both the Book and API Reference describe `main`. The installation guide covers the latest published prerelease and installation from `main`; use `main` for features that have not been released yet. Rust and Python SDK versions are managed independently.

| SDK | Package | API Reference |
|:----|:--------|:--------------|
| Python | [![PyPI](https://img.shields.io/pypi/v/ommx)](https://pypi.org/project/ommx/) | [Python API Reference](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/api/index.html) |
| Rust | [![crates.io](https://img.shields.io/crates/v/ommx)](https://crates.io/crates/ommx) | [![docs.rs](https://docs.rs/ommx/badge.svg)](https://docs.rs/ommx) |

## Development

See [DEVELOPMENT.md](./DEVELOPMENT.md) to work on OMMX, or the [Adapter API Reference](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/autoapi/ommx/adapter/index.html#ommx.adapter.SolverAdapter) to connect your own solving tool. The [Rust-to-Python PyO3 bridge](https://crates.io/crates/ommx-pyo3-bridge) has its own [API Reference](https://docs.rs/ommx-pyo3-bridge).

[![CodSpeed Badge](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://codspeed.io/Jij-Inc/ommx?utm_source=badge)

## License
© 2024 Jij Inc.

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

## Acknowledgement

<img src="./BRIDGE.png" alt="BRIDGE" width="300">

This work was performed for Council for Science, Technology and Innovation (CSTI), Cross-ministerial Strategic Innovation Promotion Program (SIP), "Promoting the application of advanced quantum technology platforms to social issues"(Funding agency : QST).
