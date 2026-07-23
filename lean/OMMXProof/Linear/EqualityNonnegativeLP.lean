import OMMXProof.State
import Mathlib.Data.Fintype.BigOperators

/-!
# Equality-form LP with nonnegative variables

An `EqualityNonnegativeLP m n` represents

`minimize cᵀx` subject to `Ax = b` and `x ≥ 0`,

with `m` equality constraints and `n` variables.
-/

namespace OMMXProof

structure EqualityNonnegativeLP (m n : Nat) where
  objective : Fin n → Rat
  matrix : Fin m → Fin n → Rat
  rhs : Fin m → Rat

namespace EqualityNonnegativeLP

def Feasible (lp : EqualityNonnegativeLP m n) (state : State n) : Prop :=
  (∀ j, 0 ≤ state j) ∧
    ∀ i, ∑ j, lp.matrix i j * state j = lp.rhs i

instance (lp : EqualityNonnegativeLP m n) (state : State n) :
    Decidable (lp.Feasible state) := by
  unfold Feasible
  infer_instance

def ObjectiveValue (lp : EqualityNonnegativeLP m n) (state : State n) : Rat :=
  ∑ j, lp.objective j * state j

end EqualityNonnegativeLP

end OMMXProof
