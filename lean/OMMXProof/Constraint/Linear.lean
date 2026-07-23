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

def IndependentAt (constraint : LinearConstraint n) (index : Fin n) : Prop :=
  constraint.expr.IndependentAt index

instance (constraint : LinearConstraint n) (index : Fin n) :
    Decidable (constraint.IndependentAt index) := by
  unfold IndependentAt
  infer_instance

def IndependentOf (constraint : LinearConstraint n)
    (privateSet : Finset (Fin n)) : Prop :=
  constraint.expr.IndependentOf privateSet

theorem holds_iff_of_independentOf {constraint : LinearConstraint n}
    {privateSet : Finset (Fin n)} {lhs rhs : State n}
    (hindependent : constraint.IndependentOf privateSet)
    (hagree : AgreeOutside privateSet lhs rhs) :
    constraint.Holds lhs ↔ constraint.Holds rhs := by
  have heval := Affine.eval_eq_of_independentOf hindependent hagree
  unfold Holds
  cases constraint.sense
  · exact heval ▸ Iff.rfl
  · exact heval ▸ Iff.rfl

def substitute (constraint : LinearConstraint n) (index : Fin n) (value : Rat) :
    LinearConstraint n where
  expr := constraint.expr.substitute index value
  sense := constraint.sense

def Same (lhs rhs : LinearConstraint n) : Prop :=
  lhs.sense = rhs.sense ∧ Affine.Same lhs.expr rhs.expr

instance (lhs rhs : LinearConstraint n) : Decidable (Same lhs rhs) := by
  unfold Same
  infer_instance

def same (lhs rhs : LinearConstraint n) : Bool := decide (Same lhs rhs)

theorem same_sound {lhs rhs : LinearConstraint n} (hcheck : same lhs rhs = true) :
    lhs = rhs := by
  have hsame : Same lhs rhs := by
    simpa [same, decide_eq_true_eq] using hcheck
  cases lhs with
  | mk lhsExpr lhsSense =>
    cases rhs with
    | mk rhsExpr rhsSense =>
      simp only [Same] at hsame
      rcases hsame with ⟨hsense, hexpr⟩
      subst rhsSense
      have : lhsExpr = rhsExpr := Affine.same_iff.mp hexpr
      subst rhsExpr
      rfl

theorem substitute_holds_iff {constraint : LinearConstraint n} {index : Fin n}
    {value : Rat} {state : State n}
    (hvalue : state index = value) :
    (constraint.substitute index value).Holds state ↔
      constraint.Holds state := by
  cases constraint with
  | mk expr sense =>
    cases sense <;> simp [substitute, Holds, Affine.eval_substitute hvalue]

end LinearConstraint

end OMMXProof
