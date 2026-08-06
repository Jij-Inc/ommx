import OMMXProof.Domain
import OMMXProof.State
import Mathlib.Algebra.BigOperators.Ring.Finset

/-!
# OneHot constraint semantics

A OneHot constraint requires every member to be binary and their sum to be one.
-/

namespace OMMXProof

structure OneHotConstraint (n : Nat) where
  members : Finset (Fin n)

namespace OneHotConstraint

def Holds (constraint : OneHotConstraint n) (state : State n) : Prop :=
  (∀ i ∈ constraint.members, state i ∈ Domain.binary) ∧
    ∑ i ∈ constraint.members, state i = 1

instance (constraint : OneHotConstraint n) (state : State n) :
    Decidable (constraint.Holds state) := by
  unfold Holds
  infer_instance

end OneHotConstraint

end OMMXProof
