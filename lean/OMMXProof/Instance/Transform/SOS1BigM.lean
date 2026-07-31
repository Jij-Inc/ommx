import OMMXProof.Instance.Transform.SOS1BigM.Formulation
import OMMXProof.Instance.Transform.SOS1BigM.CanonicalRows
import OMMXProof.Instance.Transform.SOS1BigM.Lowering
import OMMXProof.Instance.Transform.SOS1BigM.Promotion

/-!
# SOS1 Big-M transformations

This module collects both directions of the SOS1 Big-M formulation:

- `SOS1BigM.CanonicalRows` specifies the selector layout and canonical regular
  rows shared by both directions.
- `SOS1BigM.lowering` replaces one first-class SOS1 constraint with binary
  selectors, Big-M links, and a selector-cardinality row.
- `SOS1BigM.promotion` recognizes that standard Big-M layout and replaces it
  with one first-class SOS1 constraint while preserving existing special
  constraints which do not reference the removed selectors.

The promotion checker deliberately accepts a conservative sufficient
condition. Other valid formulations of SOS1 remain outside this Big-M-specific
transform family.
-/
