# Reference: API and interchange contracts

Use this entry to check exact arguments, applicability conditions, formulas, errors, and mutation behavior.
User Guides teach concepts, Tutorials provide a first success, and How-tos complete independent tasks.

## Python SDK and Adapters

- [Generated Python API Reference](../api/index): APIs from the same development revision as this Book.
- [Public b7 SDK and Adapter implementation](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/python):
  version-pinned references for execution with public `3.0.0b7`.
- [Function evaluation and representation contracts](function_contract.md): numerical boundaries and representation details.
- [Adapter implementation contract](../developer_guide/adapter.md): exact input, encoding/decoding, and evaluation target.
- [Public b7 Attachment Codec Protocol](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/python/ommx/ommx/experiment/attachments.py):
  the calling contract for converting external types to bytes and restoring them under the same Media Type.

For example, {py:meth}`~ommx.Instance.prepare` specifies exact transformation order, conditions, and errors;
{py:meth}`~ommx.Instance.convert_sos1_to_constraints` specifies SOS1 Big-M formulas and conditions.
Also consult [Versions and environments](../getting_started/compatibility.md) to avoid mixing public examples
with development APIs.

## Rust SDK

See the [Rust SDK `3.0.0-beta.7` API](https://docs.rs/ommx/3.0.0-beta.7/ommx/).
Python and Rust SDKs are versioned independently. Rust users can reach common concepts and Rust APIs
from the [Integration Developer Guide](../developer_guide/integration.md) without going through Python Tutorials.
When targeting a development implementation, consult source at the same commit as your dependency.

## Language-independent formats

| Component | Specification or definition corresponding to public b7 |
|---|---|
| OMMX Message | [Protocol Buffers definitions](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/proto/ommx): problem, candidate, and result wire schemas |
| Function wire representation | [v1 Function definition](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/proto/ommx/v1/function.proto), also used by v2 problems |
| OMMX Artifact | [Artifact specification](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/rust/ommx/doc/artifact_design.md): Manifest, Media Type, archive, and registry interoperability |
| Experiment | [Public b7 Experiment implementation and Config contract](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/rust/ommx/src/experiment): record structure inside an Artifact |

Wire-schema `v1` and `v2` are not Python import namespaces or package versions.
Artifact Media Type versions are also separate from versions of contained Instance/Solution payloads.
When handling files in another language, follow the relevant Media Types and specifications above;
Python Tutorial code is not a substitute for a format definition.

```{toctree}
:maxdepth: 1
:hidden:

function_contract
```
