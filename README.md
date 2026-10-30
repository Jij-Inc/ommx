# OMMX

[![Docs/en](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/)
[![Docs/ja](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/)
[![Discord-EN](https://img.shields.io/badge/Discord-English-default?logo=Discord)](https://discord.gg/QPMbdJCbAB)
[![Discord-JP](https://img.shields.io/badge/Discord-日本語-default?logo=Discord)](https://discord.gg/CPd9ZsxtR5)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/Jij-Inc/ommx)

OMMX (Open Mathematical prograMming eXchange) is building a shared foundation for mathematical optimization: a common representation of problems and candidates, with reusable preprocessing, transformations, and evaluation in Python and Rust.

Optimization techniques are often implemented inside a particular solver or repeated in each tool integration. OMMX aims to make those techniques reusable across modeling tools and solving methods. A new method should be able to build on accumulated problem-processing techniques, while improvements to those techniques should benefit the tools already using them.

## Why use OMMX?

Use OMMX when you want to reuse problem processing as you change models, solving methods, or the source of candidate solutions. The current v3 development line provides:

- **Transform a shared problem representation.** Apply supported operations such as integer encoding, special-constraint lowering, and bound tightening without implementing them separately for each solver integration.
- **Evaluate candidates independently of how they were obtained.** Compute objective values and constraint satisfaction for values from a solver, a heuristic, or your own code. Reconstruct recorded fixed and dependent variable values to interpret results after transformations.
- **Connect to solvers and samplers through adapters.** Adapters translate supported problem representations into backend inputs and return results in OMMX's common model.
- **Keep problems and results available for later use.** Optional Artifacts package data with metadata; Experiments record trials, settings, and results for comparison and sharing.

For example, a requirement that at most one variable is nonzero can remain an explicit SOS1 constraint in OMMX. A supporting adapter can pass it to a solver that handles SOS1 directly. For another solver, OMMX can convert it into ordinary constraints when the variable domains and finite bounds meet the conversion's requirements. Keeping this structure lets you choose when to convert it and reuse the conversion across integrations.

You still choose the mathematical model, solving method, and transformation policy, including any penalty weights. Each transformation has its own applicability conditions and guarantees: recovering variable values does not establish feasibility, and evaluating a feasible candidate does not prove optimality. The Book explains these choices; the API Reference specifies the individual operations.

The longer-term goal is to select and combine preprocessing and transformations so that more of today's manual problem preparation can be delegated to this shared foundation. The current implementation supplies specific operations and preparation policies; general automatic selection of transformation sequences remains a development direction.

## Find your next step

The **Book** explains OMMX's concepts, capabilities, and choices, with examples you can follow independently. The **API Reference** provides detailed calls, conditions, and implementation recipes.

| Purpose | Start here |
|:--------|:-----------|
| Understand OMMX and explore its capabilities | [English Book](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/) · [日本語Book](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/) |
| Choose a solver or sampler integration | [Adapter packages and input contracts](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/api/adapters.html) |
| Install the Python SDK and an optional adapter | [Installation guide](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/getting_started/install.html) · [日本語](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/getting_started/install.html) |
| Inspect and exchange saved data from the command line | [OMMX CLI](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/user_guide/cli.html) |

Until the 3.0.0 stable release, both the Book and API Reference describe `main`. Follow the installation guide to use the SDK and adapters from the same checkout. Rust and Python SDK versions are managed independently.

| SDK | Package | API Reference |
|:----|:--------|:--------------|
| Python | [![PyPI](https://img.shields.io/pypi/v/ommx)](https://pypi.org/project/ommx/) | [Python API Reference](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/api/index.html) |
| Rust | [![crates.io](https://img.shields.io/crates/v/ommx)](https://crates.io/crates/ommx) | [![docs.rs](https://docs.rs/ommx/badge.svg)](https://docs.rs/ommx) |

## Development

See [DEVELOPMENT.md](./DEVELOPMENT.md) to work on OMMX, or the [adapter implementation guide](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html) to connect your own solving tool. The [Rust-to-Python PyO3 bridge](https://crates.io/crates/ommx-pyo3-bridge) has its own [API Reference](https://docs.rs/ommx-pyo3-bridge).

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
