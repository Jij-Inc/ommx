import Mathlib.Algebra.Order.Ring.Rat
import Mathlib.Algebra.BigOperators.Ring.Finset
import Mathlib.Data.Fintype.BigOperators
import Mathlib.Tactic.Ring

/-!
# Exact semantic core

This module defines the input language for implementation-independent
semantics. It deliberately has no OMMX Rust, protobuf, floating-point,
lifecycle, or identifier semantics.
-/

namespace OMMXProof

/- OMMX State, Rational (Real in Rust impl) assignment for each decision variable ID -/
abbrev State (n : Nat) := Fin n → Rat

structure Affine (n : Nat) where
  coeff : Fin n → Rat
  constant : Rat

namespace Affine

def zero : Affine n where
  coeff := fun _ => 0
  constant := 0

def add (lhs rhs : Affine n) : Affine n where
  coeff := fun i => lhs.coeff i + rhs.coeff i
  constant := lhs.constant + rhs.constant

def neg (expr : Affine n) : Affine n where
  coeff := fun i => -expr.coeff i
  constant := -expr.constant

def sub (lhs rhs : Affine n) : Affine n := add lhs (neg rhs)

def scale (scalar : Rat) (expr : Affine n) : Affine n where
  coeff := fun i => scalar * expr.coeff i
  constant := scalar * expr.constant

def eval (expr : Affine n) (state : State n) : Rat :=
  (∑ i, expr.coeff i * state i) + expr.constant

/-- A coefficient-free affine expression evaluates to its constant exactly
once, independently of the state-space dimension. -/
theorem eval_eq_constant_of_coeff_eq_zero {expr : Affine n}
    (hzero : ∀ i, expr.coeff i = 0) (state : State n) :
    eval expr state = expr.constant := by
  simp [eval, hzero]

/-- Executable extensional equality for a finite affine expression. -/
def Same (lhs rhs : Affine n) : Prop :=
  lhs.constant = rhs.constant ∧ ∀ i, lhs.coeff i = rhs.coeff i

instance (lhs rhs : Affine n) : Decidable (Same lhs rhs) := by
  unfold Same
  infer_instance

def same (lhs rhs : Affine n) : Bool := decide (Same lhs rhs)

theorem same_iff {lhs rhs : Affine n} : Same lhs rhs ↔ lhs = rhs := by
  constructor
  · rintro ⟨hconstant, hcoeff⟩
    cases lhs with
    | mk lhsCoeff lhsConstant =>
      cases rhs with
      | mk rhsCoeff rhsConstant =>
        simp only at hconstant hcoeff
        subst rhsConstant
        have : lhsCoeff = rhsCoeff := funext hcoeff
        subst rhsCoeff
        rfl
  · intro h
    subst rhs
    exact ⟨rfl, fun _ => rfl⟩

theorem same_sound {lhs rhs : Affine n} (hcheck : same lhs rhs = true) :
    lhs = rhs := by
  apply same_iff.mp
  simpa [same, decide_eq_true_eq] using hcheck

/-- `source` implies `target` when both denote rows `eval ≤ 0`.
The coefficient identity is exact, while the target may have scalar slack. -/
def Implies (source target : Affine n) : Prop :=
  (∀ i, source.coeff i = target.coeff i) ∧
    target.constant ≤ source.constant

instance (source target : Affine n) : Decidable (Implies source target) := by
  unfold Implies
  infer_instance

theorem eval_le_of_implies {source target : Affine n}
    (h : Implies source target) (state : State n) :
    target.eval state ≤ source.eval state := by
  unfold eval
  have hsum :
      (∑ i, target.coeff i * state i) =
        ∑ i, source.coeff i * state i := by
    apply Finset.sum_congr rfl
    intro i _
    rw [h.1 i]
  rw [hsum]
  simpa [add_comm] using
    add_le_add_left h.2 (∑ i, source.coeff i * state i)

@[simp]
theorem eval_zero (state : State n) :
    eval (zero : Affine n) state = 0 := by
  simp [eval, zero]

@[simp]
theorem eval_add (lhs rhs : Affine n) (state : State n) :
    eval (add lhs rhs) state = eval lhs state + eval rhs state := by
  simp only [eval, add, add_mul, Finset.sum_add_distrib]
  ring

@[simp]
theorem eval_neg (expr : Affine n) (state : State n) :
    eval (neg expr) state = -eval expr state := by
  simp only [eval, neg, neg_mul, Finset.sum_neg_distrib]
  ring

@[simp]
theorem eval_sub (lhs rhs : Affine n) (state : State n) :
    eval (sub lhs rhs) state = eval lhs state - eval rhs state := by
  simp [sub, sub_eq_add_neg]

@[simp]
theorem eval_scale (scalar : Rat) (expr : Affine n)
    (state : State n) :
    eval (scale scalar expr) state = scalar * eval expr state := by
  simp only [eval, scale, mul_assoc, ← Finset.mul_sum]
  ring

/-- The affine expression selecting one coordinate. -/
def coordinate (index : Fin n) : Affine n where
  coeff := fun i => if i = index then 1 else 0
  constant := 0

@[simp]
theorem eval_coordinate (index : Fin n) (state : State n) :
    eval (coordinate index) state = state index := by
  simp [eval, coordinate]

/-- Exact substitution of one coordinate by a rational constant. The coordinate
space is retained; the substituted coefficient becomes zero. -/
def substitute (expr : Affine n) (index : Fin n) (value : Rat) : Affine n where
  coeff := fun i => if i = index then 0 else expr.coeff i
  constant := expr.constant + expr.coeff index * value

theorem eval_substitute {expr : Affine n} {index : Fin n} {value : Rat}
    {state : State n} (hvalue : state index = value) :
    eval (substitute expr index value) state = eval expr state := by
  classical
  simp only [eval, substitute, ite_mul, zero_mul]
  rw [Finset.sum_ite]
  simp only [Finset.sum_const_zero, zero_add]
  rw [Finset.filter_ne']
  rw [← hvalue]
  have hsum :
      (∑ x ∈ Finset.univ.erase index, expr.coeff x * state x) +
          expr.coeff index * state index =
        ∑ x, expr.coeff x * state x :=
    Finset.sum_erase_add Finset.univ
      (fun x => expr.coeff x * state x) (Finset.mem_univ index)
  calc
    (∑ x ∈ Finset.univ.erase index, expr.coeff x * state x) +
          (expr.constant + expr.coeff index * state index) =
        ((∑ x ∈ Finset.univ.erase index, expr.coeff x * state x) +
          expr.coeff index * state index) + expr.constant := by ring
    _ = (∑ x, expr.coeff x * state x) + expr.constant :=
      congrArg (fun total => total + expr.constant) hsum

end Affine

inductive VariableKind where
  | continuous
  | integer
  | binary
  deriving DecidableEq, Repr

structure Bounds where
  lower : Option Rat := none
  upper : Option Rat := none
  deriving DecidableEq, Repr

namespace Bounds

def Holds (bounds : Bounds) (value : Rat) : Prop :=
  (match bounds.lower with | none => True | some lower => lower ≤ value) ∧
  (match bounds.upper with | none => True | some upper => value ≤ upper)

instance (bounds : Bounds) (value : Rat) : Decidable (Holds bounds value) := by
  unfold Holds
  cases bounds.lower <;> cases bounds.upper <;> infer_instance

end Bounds

structure VariableDomain where
  kind : VariableKind := .continuous
  bounds : Bounds := {}
  deriving DecidableEq, Repr

namespace VariableDomain

def KindHolds (kind : VariableKind) (value : Rat) : Prop :=
  match kind with
  | .continuous => True
  | .integer => value.den = 1
  | .binary => value = 0 ∨ value = 1

def Holds (domain : VariableDomain) (value : Rat) : Prop :=
  KindHolds domain.kind value ∧ domain.bounds.Holds value

instance (kind : VariableKind) (value : Rat) : Decidable (KindHolds kind value) := by
  cases kind <;> unfold KindHolds <;> infer_instance

instance (domain : VariableDomain) (value : Rat) : Decidable (Holds domain value) := by
  unfold Holds KindHolds
  cases domain.kind <;> cases domain.bounds.lower <;> cases domain.bounds.upper <;>
    infer_instance

theorem binary_zero : KindHolds .binary 0 := by simp [KindHolds]

theorem binary_one : KindHolds .binary 1 := by simp [KindHolds]

end VariableDomain

inductive OptimizationSense where
  | minimize
  | maximize
  deriving DecidableEq, Repr

/-- A normalized linear system `inequalities i ≤ 0`, `equalities j = 0`. -/
structure LinearSystem (n : Nat) where
  ineqCount : Nat
  eqCount : Nat
  inequalities : Fin ineqCount → Affine n
  equalities : Fin eqCount → Affine n

namespace LinearSystem

def Feasible (system : LinearSystem n) (state : State n) : Prop :=
  (∀ i, (system.inequalities i).eval state ≤ 0) ∧
  (∀ i, (system.equalities i).eval state = 0)

instance (system : LinearSystem n) (state : State n) :
    Decidable (Feasible system state) := by
  unfold Feasible
  infer_instance

end LinearSystem

inductive ConstraintSense where
  | lessEqual
  | equal
  deriving DecidableEq, Repr

structure LinearConstraint (n : Nat) where
  expr : Affine n
  sense : ConstraintSense

namespace LinearConstraint

/-- Version-1 normalization of a two-sided affine row: move the right-hand
side to the left and retain only the normalized `≤ 0` / `= 0` sense. -/
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

inductive IndicatorPolarity where
  | activeOnZero
  | activeOnOne
  deriving DecidableEq, Repr

namespace IndicatorPolarity

def activeValue : IndicatorPolarity → Rat
  | .activeOnZero => 0
  | .activeOnOne => 1

def inactiveValue : IndicatorPolarity → Rat
  | .activeOnZero => 1
  | .activeOnOne => 0

def Active (polarity : IndicatorPolarity) (value : Rat) : Prop :=
  value = polarity.activeValue

instance (polarity : IndicatorPolarity) (value : Rat) :
    Decidable (Active polarity value) := by
  unfold Active
  infer_instance

theorem active_or_inactive_of_binary {polarity : IndicatorPolarity} {value : Rat}
    (hbinary : VariableDomain.KindHolds .binary value) :
    Active polarity value ∨ value = polarity.inactiveValue := by
  rcases hbinary with rfl | rfl <;> cases polarity <;> simp [Active, activeValue, inactiveValue]

end IndicatorPolarity

inductive SpecialConstraint (n : Nat) where
  | oneHot (members : Finset (Fin n))
  | indicator (trigger : Fin n) (polarity : IndicatorPolarity)
      (body : LinearConstraint n)
  | sos1 (members : Finset (Fin n))

namespace SpecialConstraint

def Holds : SpecialConstraint n → State n → Prop
  | .oneHot members, state =>
      (∀ i ∈ members, VariableDomain.KindHolds .binary (state i)) ∧
        ∑ i ∈ members, state i = 1
  | .indicator trigger polarity body, state =>
      polarity.Active (state trigger) → body.Holds state
  | .sos1 members, state =>
      ∀ i ∈ members, ∀ j ∈ members,
        state i ≠ 0 → state j ≠ 0 → i = j

instance (constraint : SpecialConstraint n) (state : State n) :
    Decidable (Holds constraint state) := by
  cases constraint <;> unfold Holds <;> infer_instance

end SpecialConstraint

/-- Exact implementation-independent model. Stable IDs, lifecycle, and
serialization belong to the future OMMX integration boundary and are
intentionally absent. -/
structure CoreModel (n : Nat) where
  domains : Fin n → VariableDomain
  linear : LinearSystem n
  specialConstraints : List (SpecialConstraint n) := []
  objective : Affine n
  sense : OptimizationSense

namespace CoreModel

def Feasible (model : CoreModel n) (state : State n) : Prop :=
  (∀ i, (model.domains i).Holds (state i)) ∧
    model.linear.Feasible state ∧
    ∀ constraint ∈ model.specialConstraints, constraint.Holds state

def ObjectiveValue (model : CoreModel n) (state : State n) : Rat :=
  model.objective.eval state

theorem linearFeasible_of_feasible {model : CoreModel n} {state : State n}
    (h : model.Feasible state) : model.linear.Feasible state := h.2.1

end CoreModel

end OMMXProof
