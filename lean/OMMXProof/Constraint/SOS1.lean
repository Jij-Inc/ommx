import OMMXProof.State
import Mathlib.Data.Finset.Basic

/-!
# SOS1 constraint semantics

An SOS1 constraint permits at most one nonzero member.
-/

namespace OMMXProof

structure SOS1Constraint (n : Nat) where
  members : Finset (Fin n)

namespace SOS1Constraint

def Holds (constraint : SOS1Constraint n) (state : State n) : Prop :=
  ∀ i ∈ constraint.members, ∀ j ∈ constraint.members,
    state i ≠ 0 → state j ≠ 0 → i = j

instance (constraint : SOS1Constraint n) (state : State n) :
    Decidable (constraint.Holds state) := by
  unfold Holds
  infer_instance

end SOS1Constraint

end OMMXProof
