import OMMXProof.Instance.Transform.SOS1BigM.SelectorFormulation
import OMMXProof.Instance.Transform.SOS1BigM.Lowering
import OMMXProof.Instance.Transform.SOS1BigM.Promotion

/-!
# SOS1 Big-M transformations

This module collects both directions of the SOS1 Big-M formulation:

## Shared layers

- `SOS1BigM.SelectorFormulation` defines the direction-independent selector
  formulation. It owns the abstract mixed reused/fresh selector semantics, the
  concrete retained-prefix/fresh-selector-suffix layout, and the canonical
  Big-M link and cardinality rows. It proves both the relation to SOS1 and the
  equivalence between the generated rows and the abstract semantics.

## Transformation directions

- `SOS1BigM.lowering` replaces one first-class SOS1 constraint with binary
  selectors, Big-M links, and a selector-cardinality row.
- `SOS1BigM.promotion` recognizes that standard Big-M layout and replaces it
  with one first-class SOS1 constraint while preserving existing special
  constraints which do not reference the removed selectors.

The primary source-to-target interpretations use the shared layers in opposite
orders, and the reverse feasibility arguments traverse the same bridge backwards:

- Lowering constructs canonical selector semantics from SOS1 and realizes those
  semantics as target rows. For the reverse feasibility implication, it
  interprets the target rows and projects the resulting selector semantics to
  SOS1.
- Promotion recognizes and interprets the source row suffix using witnessed
  link bounds, checks that the promoted member domains are contained in those
  bounds, then projects the resulting selector semantics to the promoted SOS1
  constraint. Its canonical decoding argument proceeds from SOS1 back to the
  source rows.

The promotion checker deliberately accepts a conservative sufficient
condition. Other valid formulations of SOS1 remain outside this Big-M-specific
transform family.
-/
