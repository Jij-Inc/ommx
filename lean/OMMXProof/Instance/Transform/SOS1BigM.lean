import OMMXProof.Instance.Transform.SOS1BigM.Formulation
import OMMXProof.Instance.Transform.SOS1BigM.CanonicalRows
import OMMXProof.Instance.Transform.SOS1BigM.Lowering
import OMMXProof.Instance.Transform.SOS1BigM.Promotion

/-!
# SOS1 Big-M transformations

This module collects both directions of the SOS1 Big-M formulation:

## Shared layers

- `SOS1BigM.Formulation` defines the abstract member-indexed semantics: mixed
  reused/fresh selectors, optional Big-M links, selector cardinality, and the
  equivalence with SOS1 under valid bounds and binary reused members.
- `SOS1BigM.CanonicalRows` realizes that semantics in a concrete retained-prefix
  and fresh-selector-suffix layout. It constructs the canonical link and
  cardinality rows and proves that satisfying them is equivalent to satisfying
  `SOS1BigM.Formulation`, assuming selector binary domains.

Thus `Formulation` owns the mathematical meaning, while `CanonicalRows` owns
its canonical `LinearConstraint` representation. Neither layer chooses a
transformation direction.

## Transformation directions

- `SOS1BigM.lowering` replaces one first-class SOS1 constraint with binary
  selectors, Big-M links, and a selector-cardinality row.
- `SOS1BigM.promotion` recognizes that standard Big-M layout and replaces it
  with one first-class SOS1 constraint while preserving existing special
  constraints which do not reference the removed selectors.

The primary source-to-target interpretations use the shared layers in opposite
orders, and the reverse feasibility arguments traverse the same bridge backwards:

- Lowering uses `Formulation` to construct canonical selector semantics from
  SOS1, then uses `CanonicalRows` to realize those semantics as target rows.
  For the reverse feasibility implication, target rows are interpreted through
  `CanonicalRows` and then projected to SOS1 through `Formulation`.
- Promotion uses `CanonicalRows` to recognize and interpret the source row
  suffix, then uses `Formulation` to project it to the promoted SOS1 constraint.
  Its canonical decoding argument proceeds from SOS1 through `Formulation` back
  to the source rows through `CanonicalRows`.

The promotion checker deliberately accepts a conservative sufficient
condition. Other valid formulations of SOS1 remain outside this Big-M-specific
transform family.
-/
