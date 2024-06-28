OMMX Message
=============

[![doc](https://img.shields.io/badge/Protocol-Documentation-blue)](https://jij-inc.github.io/ommx/protobuf.html)

## Why OMMX Message based on Protocol Buffers? Why not [JSON](https://www.json.org/json-en.html), [CBOR](https://cbor.io/), or [HDF5](https://www.hdfgroup.org/solutions/hdf5/)?

:zap: We need to define a data schema for messages exchanged between applications, services, and databases that make up OROps.

We have to discuss the following points to answer the question.

- Where OMMX is used?
- Why schema is required?
- Text-based or Binary-based?
- Why Protocol Buffers?

Note that this is a **better** selection, not the **best** selection. We have to keep considering these points and change the selection if necessary.

### Where OMMX is used?

Mathematical programming has been studied long time in academic fields, and many software tools have been developed to improve the solving capability. However, these tools are basically designed to use in a research process. When we try to use these invaluable tools in a real-world business process, we call it "OROps" (OR = operations research) like "MLOps" in machine-learning field, we have to face many problems. The most significant problem is the lack of interoperability between tools. We have to convert data between tools, and it is a time-consuming and error-prone process. OMMX is designed to solve this problem.

### Why schema is required?
Different from research process where few researchers use and create few tools, in OROps, many developers use and create many tools. The interoperability between tools becomes significant issue due to this point. It is possible for human to manage input and output data stored in general purpose data format like JSON or HDF5 for few tools, but it is hard for many tools. Thus, we have to introduce machine-readable schema.

### Text-based or Binary-based?
TBW

### Why Protocol Buffers?
TBW

## Compatibility

- OMMX defines a protocol buffers schema with version like `v1`, `v2`, etc. `v1` schema has a namesapce `ommx.v1`.
- Schemas in `ommx.v1` will be compatible after [ommx.v1 schema release](https://github.com/Jij-Inc/ommx/milestone/3). Note that the schema can be changed incompatible way until this release.
- `v2` schema with namespace `ommx.v2` will start developing if we need to change the schema in incompatible way after `ommx.v1` release. Compatible changes will be made in `v1` schema also after its release. We never create namespaces like `ommx.v1_1`.
