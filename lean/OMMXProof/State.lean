import Mathlib.Algebra.Order.Ring.Rat
import Mathlib.Data.Finset.Basic

/-!
# Exact state semantics

This module defines the exact semantic counterpart of OMMX `State`. The SDK
stores a sparse real-valued assignment keyed by decision-variable IDs; this
independent model instead assigns an exact rational value to every coordinate
in a finite decision-variable space.
-/

namespace OMMXProof

/-- OMMX `State` as a total rational assignment over decision variable indices.
Different from SDK which allows non-contiguous ID e.g. {1: 0.1, 3: 0.2},
this model assumes indices are packed in `Fin n`.
-/
abbrev State (n : Nat) := Fin n → Rat

/-- Two states agree on every component outside `privateSet`. -/
def AgreeOutside (privateSet : Finset (Fin n))
    (lhs rhs : State n) : Prop :=
  ∀ i, i ∉ privateSet → lhs i = rhs i

end OMMXProof
