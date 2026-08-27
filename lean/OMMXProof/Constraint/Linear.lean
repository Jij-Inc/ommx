import OMMXProof.Domain
import OMMXProof.Function.Affine

/-!
# Linear constraint semantics

A linear constraint is one normalized affine equality or inequality.
-/

namespace OMMXProof

inductive ConstraintSense where
  | lessEqual
  | equal
  deriving DecidableEq, Repr

structure LinearConstraint (n : Nat) where
  expr : Affine n
  sense : ConstraintSense

namespace LinearConstraint

/-- Move the right-hand side to the left and retain the normalized
`≤ 0` / `= 0` sense. -/
def normalize (lhs rhs : Affine n) (sense : ConstraintSense) :
    LinearConstraint n where
  expr := lhs.sub rhs
  sense := sense

def Holds (constraint : LinearConstraint n) (state : State n) : Prop :=
  match constraint.sense with
  | .lessEqual => constraint.expr.eval state ≤ 0
  | .equal => constraint.expr.eval state = 0

instance (constraint : LinearConstraint n) (state : State n) :
    Decidable (Holds constraint state) := by
  unfold Holds
  cases constraint.sense <;> infer_instance

theorem normalize_holds_iff (lhs rhs : Affine n) (sense : ConstraintSense)
    (state : State n) :
    (normalize lhs rhs sense).Holds state ↔
      match sense with
      | .lessEqual => lhs.eval state ≤ rhs.eval state
      | .equal => lhs.eval state = rhs.eval state := by
  cases sense <;> simp [normalize, Holds, Affine.eval_sub, sub_eq_zero]

end LinearConstraint

end OMMXProof
