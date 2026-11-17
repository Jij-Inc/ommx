# Developing OMMX

Choose tasks by the component you are changing. Task namespaces express
management ownership; CI groups those tasks by execution environment and build
reuse. Rust implementations of Python APIs belong to the Python SDK.

Install [Task](https://taskfile.dev/installation/) and run commands from the
repository root. List available entry points with:

```shell
task -l
```

The root `Taskfile.yml` includes each area's Taskfile. Running Task from a
subdirectory can select that directory's Taskfile instead, changing the
available names. The commands below use the root namespaces.

## Code ownership

| Area | Sources | Task namespace |
|:-----|:--------|:---------------|
| Rust SDK | `rust/ommx/`, `rust/ommx-derive/`, including the SDK's CLI | `rust:*` |
| Python SDK | `python/ommx/`, including the `_ommx_rust` crate | `python:ommx:*` |
| Python interoperability | `python/ommx-pyo3-bridge/`, including its fixture | `python:bridge:*` |
| Solver adapters | `python/ommx-*-adapter/` | `python:<adapter>:*` |
| Shared Python development | `python/scripts/`, root `conftest.py`, SDK/bridge/adapter aggregation | `python:*` |
| Documentation | `docs/`, including Sphinx extensions and publication checks | `docs:*` |
| Protobuf | `proto/` and the generator in `rust/protogen/` | `proto:*` |
| Dataset packaging | `rust/dataset/` | `dataset:*` |
| Formal semantics | `lean/` | `lean:*` |
| CI infrastructure | `.github/` workflows and shared execution setup | `ci:*` |

The Cargo workspace contains `ommx`, `ommx-derive`, `_ommx_rust`,
`ommx-pyo3-bridge`, `ommx-pyo3-bridge-fixture`, `protogen`, and `dataset`.
Cargo workspace membership describes dependency and build relationships.
Owner-specific tasks explicitly select their packages. In particular,
`rust:test` selects `ommx` and `ommx-derive`, while a root `cargo test` can also
execute Python SDK code.

The bridge provides the contract for independently built Rust/PyO3 extensions
to return canonical Python OMMX objects. Its crate and fixture are managed with
the Python SDK even though the bridge is distributed through crates.io.

## Build and test

### Rust SDK

Install [Rust through rustup](https://www.rust-lang.org/tools/install), then run:

```shell
task rust:check
task rust:test
task rust:clippy
task rust:doc:check
```

These tasks cover `ommx` and `ommx-derive`. Native Rust SDK development requires
no OMMX Python environment, uv, or PyO3. Operation-specific requirements such as
Docker for registry E2E tests are prepared separately.

Pass Cargo arguments after `--`, for example:

```shell
task rust:test -- --features ommx/cli
```

`rust:insta`, `rust:insta:review`, and `rust:insta:accept` manage snapshots for
the Rust SDK packages. `rust:doc` generates their Rust API documentation.

`rust:check:wasm32-emscripten` uses a dedicated target toolchain. Emscripten uses
Python internally; this does not require the OMMX Python SDK environment.

### Python SDK and adapters

Install [uv](https://docs.astral.sh/uv/getting-started/installation/) and Rust,
then prepare the source-development environment:

```shell
task python:sync
```

Python packages use a [uv workspace](https://docs.astral.sh/uv/concepts/projects/workspaces/).
`python:sync` creates the virtualenv, builds the SDK, and installs development
dependencies. Run Python commands through uv or the project tasks. Use
`python:upgrade` when intentionally updating dependencies.

Choose the scope of validation:

```shell
task python:ommx:test
task python:ommx-highs-adapter:test
task python:test
```

`python:test` prepares the source-development environment once and calls the
SDK, bridge, and adapter test aggregates. A standalone package aggregate
prepares its required environment. Shared preparation is internal and avoids
rebuilding the SDK for each package.

`python:ommx:test` includes Pyright, lint, Rust implementation tests, and pytest.
`python:ommx:lint` includes both Ruff and `_ommx_rust` Clippy. The focused
`python:ommx:clippy` entry point runs only the latter.

Run the binding's Rust tests without building a Python wheel:

```shell
task python:ommx:test-rust
# Select a module using Cargo's filter:
task python:ommx:test-rust -- --lib error::tests
# Pass arguments to the test binary after a second separator:
task python:ommx:test-rust -- --lib -- --list
```

`test-rust` covers the crate's Rust tests, including test-bearing binaries. It
installs the locked `binding-test` dependency group and obtains `PYO3_PYTHON`
and `PYTHONPATH` from `uv run --no-sync python`. Embedded CPython needs the
virtualenv's module search path explicitly; running Cargo under uv alone does
not supply it. These tests use the pandas version in `uv.lock`.

Focused `pytest` and `pyright` tasks consume a prepared environment with
`uv run --no-sync`. They do not implicitly replace an installed SDK wheel with
a source build. Minimal lint and binding-test dependency synchronization also
preserves installed wheels and unrelated development tools.

The adapters for HiGHS, PySCIPOpt, Python-MIP, and OpenJij each own their tests,
typing, lint, and formatting. Package classifiers declare the Python versions
and free-threaded support used to select adapters in CI.

### Python interoperability

```shell
task python:bridge:test
```

The bridge aggregate covers lint, Rust tests, receiver independence, fixture
stub generation and typing, and transfer between separately compiled extension
modules. Focused entry points include `test-rust`, `receiver-test`, `check`,
`runtime-test`, and `doc:check` under `python:bridge:`.

Rust tests and all-target Clippy cover both `ommx-pyo3-bridge` and
`ommx-pyo3-bridge-fixture`, including the fixture's library and stub generator.
The receiver-only check uses a separate `--no-default-features` Cargo
invocation and a dependency-tree assertion. Sender-enabled consumers must not
mask receiver independence through feature unification.

Integration tests use an installed SDK and build the fixture extension locally.
When validating an SDK wheel, they preserve that wheel throughout the run.

### Documentation

Sphinx builds the [English Book](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/)
and [Japanese Book](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/), both
published on Read the Docs.

For source development, `python:sync` supplies the SDK and documentation
dependencies. Build both editions, run their code examples, or open one edition:

```shell
task docs:build
task docs:test
task docs:en:open
```

Use `docs:en:build` or `docs:ja:build` to build one edition. `docs:test` includes
code execution and entry-link validation. Run local language builds
sequentially because they share generated files; CI uses separate checkouts for
the two editions.

PyO3 docstrings supply the Python SDK's API Reference. Regenerate SDK stubs,
exports, and reference metadata with:

```shell
task python:ommx:stubgen
```

[pyo3-stub-gen](https://github.com/Jij-Inc/pyo3-stub-gen) writes reference metadata
and pages under [docs/api](./docs/api/). Sphinx AutoAPI reads adapter and utility
docstrings, and the [reference index](./docs/api/index.rst) links both sets of
pages. The SDK owns API generation; docs owns Sphinx publication. Rust SDK
rustdoc stays under `rust:*`, and bridge rustdoc under `python:bridge:*`.

### Protobuf and dataset packaging

Schemas under `proto/` use [Buf](https://buf.build/docs/introduction); see
[buf.yaml](./proto/buf.yaml). Rust code generation also requires protoc and the
Rust toolchain.

```shell
task proto:rust
task proto:test
task proto:lint
```

`proto:test` runs the `protogen` crate's tests and verifies that regeneration
matches committed outputs. `proto:lint` covers schema lint and generator Clippy.
Schema compatibility checks also belong to this area. `proto:doc` generates
schema documentation.

The `dataset` crate packages collections such as MIPLIB2017 and QPLIB as OMMX
Artifacts. Upload artifacts separately with `ommx push`.

```shell
task dataset:test
task dataset:clippy
```

### Formal semantics

The independent Lean project formalizes mathematical semantics for
proof-carrying presolve and special-constraint recovery. It does not depend on
the Rust SDK or Protobuf schema. Install Lean through `elan`, then run:

```shell
task lean:cache
task lean:check
```

`lean:check` includes import coverage, build, trust, and executable fixture
checks. See [lean/README.md](./lean/README.md) for its formal contract and scope.

## Formatting and lint

Before committing, format the management areas affected by the change:

```shell
task rust:format
task python:format
task docs:format
```

With all required environments available, format the whole repository:

```shell
task format
```

The root task only calls `rust:format`, `python:format`, `docs:format`,
`proto:format`, and `dataset:format`. File selection belongs to those individual tasks. `python:format`
aggregates SDK, bridge, adapter, and shared Python support formatting.
`proto:format` includes both schemas and the generator's Rust sources.

Each formatter has a non-mutating `format:check` counterpart; root
`format:check` delegates to them. Formatting prepares only the needed tools and
does not build the SDK. Lean has no formatting task. Documentation formatting
and Ruff lint cover Python configuration, extensions, and scripts. The generated
`docs/api/pyo3_stub_gen_ext.py` retains its upstream generator's formatting.
Documentation Markdown and reStructuredText have no prescribed automatic formatter.

`python:lint` aggregates package lint tasks and shared Python support checks.
CI can invoke focused Ruff checks separately from Clippy while preserving each
area's complete coverage.

## CI execution

### Jobs and build reuse

Task ownership and CI execution grouping serve different purposes. A runner
can execute tasks owned by several areas when their environments and build
outputs are compatible. SDK and dataset checks sharing a runner does not make
`dataset:*` part of the Rust SDK namespace.

| Execution group | Work on the same runner | Required input |
|:----------------|:------------------------|:---------------|
| Lightweight checks | Owner-specific format checks, focused Ruff checks, Buf lint, workflow validation | Only the required tools |
| Rust SDK tests | SDK tests and dataset tests | Python-free Rust environment |
| Rust SDK static checks | SDK Clippy, rustdoc, and dataset Clippy | Python-free Rust environment |
| Specialized Rust checks | Semver, authentication E2E, CLI, and Emscripten in their appropriate jobs | Operation-specific tools, services, and artifacts |
| Protobuf generation | Generator tests and Clippy, regeneration/diff checks, schema compatibility | Rust, protoc, and Buf as needed |
| Python binding validation | Binding Clippy, Rust tests, SDK stub generation/diff checks | Representative Python environment; no SDK wheel dependency |
| SDK wheel production | Build and upload Python 3.11 and 3.15 wheel variants | Build environment |
| Python typing | Pyright over SDK sources and tests | Built SDK wheel and typing dependencies |
| SDK and adapter runtime | Supported Python-version and free-threaded matrix with adapter selection | Built SDK wheel |
| Bridge validation | Crate and fixture Clippy/Rust tests, receiver independence, rustdoc, fixture stubs, typing, build, and runtime checks | Built SDK wheel; one representative Python environment |
| Documentation | English and Japanese builds, code execution, and entry-link checks on separate runners | Built SDK wheel and documentation dependencies |
| Lean | Proof, fixture, import, and trust checks | Lean environment |

```text
Rust SDK checks -------------------------- independent
Python binding tests + stub generation --- independent
SDK wheel production
  +-- Python typing
  +-- SDK / adapter runtime matrix
  +-- Bridge validation
  +-- English documentation
  +-- Japanese documentation
```

Keep binding Rust tests and SDK stub generation together, and keep fixture
construction and its checks together in bridge validation. Separate Rust-only
and Python-side validation deliberately duplicates some compilation of `ommx`
to establish environment independence. Avoid multiplying that duplication
within each side.

The wheel producer builds and uploads the SDK without running the binding test
suite or stub generation. Downstream jobs wait for the producer job to finish,
even if it uploads an artifact earlier. Consumers share same-commit wheel
artifacts within the workflow run, including the two documentation jobs.

Wheel consumers install dependencies without installing source `ommx`, install
the intended wheel, and invoke focused tasks with `uv run --no-sync`. They
verify that the downloaded wheel is still installed at the end. Pyright still
checks the SDK sources and tests; wheel reuse does not narrow validation to
installed stubs. Bridge integration builds only its fixture locally.

Lightweight checks call focused Ruff entry points, not Python lint aggregates
that also run Clippy. All-target Clippy belongs to native validation groups.
Credentials, external services, and specialized toolchains stay in dedicated
jobs. Benchmarks retain their own execution policy and measurement profiles.

### Environment setup and caches

Shared setup prepares tools and caches. Tasks and jobs choose dependency groups
and source versus wheel installation explicitly. Local actions under
`.github/actions/` hold project-specific defaults; simple tool installation
uses the corresponding upstream action directly.

| Setup responsibility | Contract |
|:---------------------|:---------|
| Rust | Select toolchain and components; restore Cargo caches for compilation/checking jobs; do not install Python |
| Python | Select uv and the interpreter and configure dependency-download caching; do not synchronize the project or build the SDK |
| Protobuf | Install Buf and/or protoc where required |
| Emscripten | Prepare its Rust target, Emscripten, and Node environment |
| Task and shared tools | Install them in jobs that invoke them |

These contracts apply to PR tests, benchmarks, releases, dependency updates,
and cloud integration. A formatter or wheel consumer that does not compile
Rust does not restore a Cargo build cache.

Caches reuse compatible dependencies across runs. Wheel artifacts transfer a
completed SDK within a run. A shared runner filesystem reuses intermediate
outputs between steps. A cache hit does not imply that the current commit's
workspace crates are already compiled. Features, ABI settings, profiles,
targets, and toolchains can require separate builds even with shared caches.
Configure the relevant toolchain and interpreter inputs before restoration.

### Supporting scripts

Scripts live with the activity they support and are included in that owner's
formatting, lint, and execution tasks.

| Location | Responsibility |
|:---------|:---------------|
| `docs/scripts/check-book-entry-links.py` | Validate published Book entry links |
| `python/scripts/select-python-adapters.py` | Select adapters from their declared Python support |
| `python/ommx/integration/gcloud-artifact/roundtrip.py` | Exercise the public Python Artifact API against Google Cloud Artifact Registry |
| `python/ommx/integration/gcloud-artifact/README.md` | Document integration resources, authentication, and execution |
| `python/ommx/integration/gcloud-artifact/cleanup.json` | Define the integration repository's artifact-retention policy |
| Root `conftest.py` | Share pytest hooks across the SDK, adapters, and bridge fixture |

The Google Cloud integration workflow runs on main and manual dispatch from
main. Its path is part of the external identity-provider condition. Authenticate
after SDK preparation so compilation does not consume the short-lived token's
lifetime, and retain evidence uploads. Cloud-policy administration and
credentialed integration execution are separate from routine formatting and PR
checks.

### Coverage and execution cost

Every Cargo member has an owner responsible for its tests, all-target Clippy,
and formatting. CI also covers rustdoc, semver, generated-output consistency,
receiver independence, and integration behavior. Select required features such
as `ommx/cli` explicitly; another workspace member enabling a feature is not a
substitute for the owner's declared coverage.

Keep the supported Python/free-threaded matrix, adapter-selection policy, both
Book editions, and specialized checks when regrouping jobs. Binding Rust tests
use one locked pandas version. Job grouping must not weaken test assertions or
change the supported inputs.

Evaluate CI changes using equivalent SDK sources, runner types, toolchains,
and Python versions under both warm and cold caches. Measure setup, dependency
installation, compilation, test execution, artifact transfer, total runner
time, and elapsed time until required checks finish separately. Distinguish
queue time from execution time. A reduction in total runner time does not by
itself justify delaying the last required check; use measured results to decide
whether related steps share a runner or run in parallel.

## Versioning and releases

### Cargo crates

`ommx`, `ommx-derive`, and `ommx-pyo3-bridge` share the Cargo workspace version
and follow [semantic versioning](https://semver.org/). Their version is
independent of the Protobuf schema version and the Python distribution version.
The bridge's Python-side ownership does not change this release coupling.

Install [cargo-edit](https://github.com/killercup/cargo-edit), then update the
shared version with:

```shell
task rust:set-version -- x.y.z
```

The [Rust release workflow](https://github.com/Jij-Inc/ommx/actions/workflows/release_rust.yml)
uses tags of the form `rust-x.y.z`. Package versions come from `Cargo.toml`.
Publication orders `ommx-derive` before `ommx`, followed by the bridge. Release
automation proposes subsequent version updates through pull requests. Rust SDK
semver checks support review of public API changes and required version bumps.

### Python distributions

The Python SDK and solver adapters share a Python distribution version:

```shell
task python:set-version -- x.y.z
```

This updates the SDK and adapter versions and their versioned dependencies.
The bridge remains a Cargo crate rather than a separate Python distribution.

The [Python release workflow](https://github.com/Jij-Inc/ommx/actions/workflows/release_python.yml)
uses tags of the form `python-x.y.z`; package versions come from
`pyproject.toml`. Release automation proposes subsequent version updates
through pull requests. Review Python API compatibility and version increments
explicitly; Python has no automatic semver check.
