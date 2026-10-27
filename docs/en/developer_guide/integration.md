# Connect another tool or language to OMMX

This page is for implementers connecting modelers, domain tools, Rust programs, or readers/writers
in other languages to OMMX. Reading all Python Tutorials is not a prerequisite.
You need the common problem/candidate semantics and the contract of the boundary you are implementing.

The common-concept Guides include Python examples, but Rust and other-language readers can use the equations, tables and explanations without executing them, then proceed to the version-pinned APIs and specifications below.

## Choose an integration boundary

| Goal | Responsibility | Next reference |
|---|---|---|
| Export a problem from a modeler | Map domains, objectives, constraints, names, and IDs to an OMMX problem | Instance, Function, special constraints |
| Manipulate problems or candidates in Rust | Construct, evaluate, and transform using public SDK types and conditions | Version-pinned Rustdoc |
| Read or write Messages in another language | Satisfy the wire schema associated with the Media Type and the problem semantics | Message definitions and conceptual Guides |
| Exchange problems, results, and analysis data together | Follow the Artifact specification for layers, types, content identifiers, and archives | Artifact specification |
| Attach an external Python type to an Experiment | Define its Codec and compatibility in the package owning the type | Codec Protocol |
| Connect an execution backend | Implement exact input, variable encoding, candidate decoding, and evaluation | Adapter Developer Guide |

You do not need to implement every boundary. A tool that only reads Messages need not implement
an Adapter or Experiment. Artifact and Experiment are not prerequisites for core operations;
they are options for storage, exchange, and recording.

## Align common semantics first

An [Instance](../user_guide/instance.md) contains problem variables, expressions, constraints, and sense;
a [Solution](../user_guide/solution.md) evaluates a candidate against that problem.
Retained variables are not necessarily all independent search variables; see [Variable roles](../user_guide/variable_roles.md).

Align which variables IDs identify, domains and kinds, constraint satisfaction conditions, and reported
objective meanings across software. Reconstructing candidate values, evaluating feasibility, and reporting
optimality are separate responsibilities. Reading values back alone does not guarantee original feasibility
or global optimality.

If using transformations, start with [InstanceClass](../user_guide/instance_class.md) and
[PreparationPolicy](../user_guide/preparation_policy.md). Membership concerns representation conditions;
[Transformation guarantees](../user_guide/transformation_guarantees.md) explains correspondence with the source.
Also separate backend coefficient formatting from transformations of the problem itself.

## Match versions when implementing

[Reference](../reference/index.md) collects pinned Python/Rust APIs and Message, Artifact, Codec, and Adapter
contracts. Python `3.0.0b7` and Rust `3.0.0-beta.7` are independent versions.
Wire-schema versions are not SDK versions either. An integration targeting a public release should not
depend on additional `main` APIs without identifying that development dependency.

When reading or writing in another language, follow root objects' public contracts as well as reading
schema fields. Add a minimal roundtrip test checking that problem/candidate correspondence, types, and
attachments are retained. Artifact receiver tests should use an empty receiving environment independent
of the sender's Registry and memory.
