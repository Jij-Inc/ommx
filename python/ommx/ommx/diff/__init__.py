"""Instance diffing utilities for OMMX.

The current API exposes the ID-mapping heuristic used as the first step toward
semantic instance diffs.
"""

from .instance_mapping import InstanceMapping, match_instance_ids

__all__ = ["InstanceMapping", "match_instance_ids"]
