import OMMXProof.State
import Mathlib.Data.Fintype.BigOperators

/-!
# Less-than-or-equal-form LP with nonnegative variables

A `LessEqualNonnegativeLP m n` represents

`maximize cᵀx` subject to `Ax ≤ b` and `x ≥ 0`,

with `m` inequality constraints and `n` variables.
-/

namespace OMMXProof

structure LessEqualNonnegativeLP (m n : Nat) where
  objective : Fin n → Rat
  matrix : Fin m → Fin n → Rat
  rhs : Fin m → Rat

namespace LessEqualNonnegativeLP

def Feasible (lp : LessEqualNonnegativeLP m n) (state : State n) : Prop :=
  (∀ j, 0 ≤ state j) ∧
    ∀ i, ∑ j, lp.matrix i j * state j ≤ lp.rhs i

instance (lp : LessEqualNonnegativeLP m n) (state : State n) :
    Decidable (lp.Feasible state) := by
  unfold Feasible
  infer_instance

def ObjectiveValue (lp : LessEqualNonnegativeLP m n) (state : State n) : Rat :=
  ∑ j, lp.objective j * state j

end LessEqualNonnegativeLP

end OMMXProof
