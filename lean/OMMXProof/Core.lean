import OMMXProof.Function.Affine

/-!
# Exact semantic core

This module defines the input language for implementation-independent
semantics. It deliberately has no OMMX Rust, protobuf, floating-point,
lifecycle, or identifier semantics.
-/

namespace OMMXProof

inductive VariableKind where
  | continuous
  | integer
  | binary
  deriving DecidableEq, Repr

/-- Bound of a decision variable.
`None` for `lower` (`upper`) means lower (upper) bound is unbounded
-/
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

/--
# Simplified semantic model for OMMX Instance.

- IDs are packed into a finite index space `Fin n`, while Rust SDK uses stable parse IDs.

## Temporal Limitations

- Only Affine expressions are supported, while Rust SDK supports arbitrary polynomial expressions.
-/
structure Instance (n : Nat) where
  domains : Fin n → VariableDomain
  linear : LinearSystem n
  specialConstraints : List (SpecialConstraint n) := []
  objective : Affine n
  sense : OptimizationSense

namespace Instance

def Feasible (inst : Instance n) (state : State n) : Prop :=
  (∀ i, (inst.domains i).Holds (state i)) ∧
    inst.linear.Feasible state ∧
    ∀ constraint ∈ inst.specialConstraints, constraint.Holds state

def ObjectiveValue (inst : Instance n) (state : State n) : Rat :=
  inst.objective.eval state

theorem linearFeasible_of_feasible {inst : Instance n} {state : State n}
    (h : inst.Feasible state) : inst.linear.Feasible state := h.2.1

end Instance

end OMMXProof
