---
applyTo: "python/**/*"
---

# Python SDK (ommx package) Development Guidelines

This document provides specific instructions for developing the Python SDK, primarily within the `ommx` package located in the `python/ommx/` directory.

## Python Package Structure
- The repository contains multiple Python packages. The core `ommx` package (located in `python/ommx/`) serves as a dependency for various adapter projects, such as `ommx-pyscipopt-adapter` and others following the `*-adapter` naming convention. Changes to the `python/ommx/` package may necessitate corresponding changes in these dependent adapter projects.
