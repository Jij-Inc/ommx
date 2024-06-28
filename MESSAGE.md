OMMX Message
=============

[![doc](https://img.shields.io/badge/Protocol-Documentation-blue)](https://jij-inc.github.io/ommx/protobuf.html)

## Why OMMX Message schema based on Protocol Buffers? Why not [JSON](https://www.json.org/json-en.html), [CBOR](https://cbor.io/), or [HDF5](https://www.hdfgroup.org/solutions/hdf5/)?

A. We need to define a data schema for messages exchanged between applications, services, and databases.


## Compatibility

- OMMX defines a protocol buffers schema with version like `v1`, `v2`, etc. `v1` schema has a namesapce `ommx.v1`.
- Schemas in `ommx.v1` will be compatible after [ommx.v1 schema release](https://github.com/Jij-Inc/ommx/milestone/3). Note that the schema can be changed incompatible way until this release.
- `v2` schema with namespace `ommx.v2` will start developing if we need to change the schema in incompatible way after `ommx.v1` release. Compatible changes will be made in `v1` schema also after its release. We never create namespaces like `ommx.v1_1`.
