import OMMXProof.Instance.Transform.SOS1BigM.Basic
import OMMXProof.Instance.Transform.SOS1BigM.Formulation
import OMMXProof.Instance.Transform.SOS1BigM.Plan
import OMMXProof.Instance.Transform.SOS1BigM.Target

/-!
# SOS1 Big-M lowering

This module is the entrypoint for the SDK SOS1 conversion as an actual
`Instance.Transform`, including the formulation, validated plans, target
construction, encode/decode maps, and lowering correctness.

## Terminology

- An **SOS1 member** is a source decision-variable component named by the
  selected SOS1 constraint.
- A **selector** is the binary value associated with an SOS1 member in the
  at-most-one formulation. A selector value of zero forces its member to zero;
  a value of one only permits the member to be nonzero.
- A **reused selector** is a binary SOS1 member used directly as its own
  selector.
- A **fresh selector** is a new binary variable appended to the target instance
  for a non-binary SOS1 member.
- The **canonical selector** is the deterministic selector used by `encode`: it
  is zero exactly when its member is zero.
- A **link constraint** is a Big-M inequality connecting a non-binary member
  `vᵢ` to its fresh selector `sᵢ`. For finite bounds `lᵢ ≤ vᵢ ≤ uᵢ`, the
  generated sides are `vᵢ ≤ uᵢ sᵢ` when `0 < uᵢ` and
  `lᵢ sᵢ ≤ vᵢ` when `lᵢ < 0`.
- The **cardinality constraint** is the at-most-one inequality over all reused
  and fresh selectors.
- A **plan** selects the occurrence of the source SOS1 constraint to lower. A
  **validated plan** additionally proves that every member has finite bounds,
  so all required Big-M coefficients can be constructed.
-/
